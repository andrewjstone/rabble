extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;
extern crate slog_envlogger;
extern crate slog_term;
extern crate log;
extern crate time;

mod utils;

use std::{str, thread};
use std::thread::JoinHandle;
use std::net::TcpStream;
use amy::{Poller, Receiver, Sender};
use slog::DrainExt;
use time::{SteadyTime, Duration};

use utils::messages::*;
use utils::replica::Replica;
use utils::api_server;
use utils::wait_for;

use rabble::{
    Pid,
    NodeId,
    Envelope,
    Msg,
    ClusterStatus,
    MsgpackSerializer,
    Serialize,
    Node,
    CorrelationId
};

const API_SERVER_IP: &'static str = "127.0.0.1:12001";

type CrNode = Node<RabbleUserMsg>;
type CrReceiver = Receiver<Envelope<RabbleUserMsg>>;
type CrSender = Sender<Envelope<RabbleUserMsg>>;

#[test]
fn chain_replication() {
    let node_ids = create_node_ids();
    let test_pid = Pid {
        name: "test-runner".to_string(),
        group: None,
        node: node_ids[0].clone()
    };

    let (nodes, mut handles) = start_nodes();

    // We create an amy channel so that we can pretend this test is a system service.
    // We register the sender with node1 so that we can check the responses to admin calls
    // like node.get_cluster_status().
    let mut poller = Poller::new().unwrap();
    let (test_tx, test_rx) = poller.get_registrar().channel().unwrap();
    nodes[0].register_system_thread(&test_pid, &test_tx).unwrap();

    let pids = create_replica_pids(&nodes);
    // We only send API requests to node1, so only bother starting an API server on this node
    let (service_pid, service_tx, service_handle) = api_server::start(nodes[0].clone());
    handles.push(service_handle);

    spawn_replicas(&nodes, &pids);

    join_nodes(&nodes, &mut poller, &test_pid, &test_rx);

    run_client_operations(&pids[0]);

    verify_histories(&pids);

    shutdown(nodes, test_pid, service_pid, service_tx);

    for h in handles {
        h.join().unwrap();
    }
}

fn start_nodes() -> (Vec<CrNode>, Vec<JoinHandle<()>>) {
    let term = slog_term::streamer().build();
    let drain = slog_envlogger::LogBuilder::new(term)
        .filter(None, slog::FilterLevel::Debug).build();
    let root_logger = slog::Logger::root(drain.fuse(), o!());
    slog_stdlog::set_logger(root_logger.clone()).unwrap();
    create_node_ids().into_iter().fold((Vec::new(), Vec::new()),
                                  |(mut nodes, mut handles), node_id| {
        let (node, handle_list) = rabble::rouse(node_id, Some(root_logger.clone()));
        nodes.push(node);
        handles.extend(handle_list);
        (nodes, handles)
    })
}

fn spawn_replicas(nodes: &Vec<CrNode>, pids: &Vec<Pid>) {
    for i in 0..pids.len() {
        let next = if i == pids.len() - 1 {
            None
        } else {
            Some(pids[i + 1].clone())
        };
        let replica = Box::new(Replica::new(pids[i].clone(), next));
        nodes[i].spawn(&pids[i], replica).unwrap();
    }
}

fn join_nodes(nodes: &Vec<CrNode>, poller: &mut Poller, test_pid: &Pid, test_rx: &CrReceiver) {
    nodes[0].join(&nodes[1].id).unwrap();
    nodes[0].join(&nodes[2].id).unwrap();
    let correlation_id = CorrelationId::pid(test_pid.clone());
    wait_for_connected_cluster(&nodes[0], poller, correlation_id, test_rx);
}

/// launch 3 clients and send concurrent operations to the head of the chain
fn run_client_operations(pid: &Pid) {
    let mut client_handles = Vec::new();
    let sleep_time = Duration::milliseconds(10);
    let timeout = Duration::seconds(5);
    for i in 0..3 {
        let pid = pid.clone();
        let h = thread::spawn(move || {
            let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
            let mut serializer = MsgpackSerializer::new();
            // TODO: Use wait_for here just in case the write buffer is too small on test machine
            assert_matches!(serializer.write_msgs(&mut sock, Some(&ApiClientMsg::Op(pid, i))),
                            Ok(true));
            sock.set_nonblocking(true).unwrap();
            assert_eq!(true, utils::wait_for(sleep_time, timeout, || {
                if let Ok(Some(ApiClientMsg::OpComplete)) = serializer.read_msg(&mut sock) {
                    return true;
                }
                false
            }));
        });
        client_handles.push(h);
    }

    for h in client_handles {
        h.join().unwrap();
    }
}

/// Verify that after all client operations have gotten replies that the history of operations in
/// each replica is identical.
fn verify_histories(pids: &Vec<Pid>) {
    let pids = pids.clone();
    let h = thread::spawn(move || {
        let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
        sock.set_nonblocking(true).unwrap();
        let mut serializer = MsgpackSerializer::new();
        let mut history = Vec::new();
        for pid in pids {
            assert_matches!(serializer.write_msgs(&mut sock,
                                                  Some(&ApiClientMsg::GetHistory(pid))),
                                                  Ok(true));
            loop {
                thread::sleep(std::time::Duration::from_millis(10));
                match serializer.read_msg(&mut sock) {
                    Ok(None) => (),
                    Ok(Some(ApiClientMsg::History(h))) => {
                        if history.len() == 0 {
                            history = h;
                        } else {
                            assert_eq!(history, h);
                            assert!(history.len() != 0);
                        }
                        break;
                    },
                    Ok(val) => {
                        println!("{:?}", val);
                        assert!(false)
                    },
                    Err(e) => {
                        println!("{}", e);
                        assert!(false)
                    }
                }
            }
        }
    });
    h.join().unwrap();
}

fn shutdown(nodes: Vec<CrNode>,
            test_pid: Pid,
            service_pid: Pid,
            service_tx: CrSender)
{
    let envelope = Envelope::new(service_pid, test_pid, Msg::Shutdown, None);
    service_tx.send(envelope).unwrap();
    for node in nodes {
        node.shutdown();
    }
}

fn wait_for_connected_cluster(node: &CrNode,
                              poller: &mut Poller,
                              correlation_id: CorrelationId,
                              test_rx: &CrReceiver) {
    let start = SteadyTime::now();
    loop {
        thread::sleep(std::time::Duration::from_millis(10));
        node.cluster_status(correlation_id.clone()).unwrap();
        // We are only polling on the test channel, so we don't need to know what woke the poller
        let _ = poller.wait(5000).unwrap();
        let envelope = test_rx.try_recv().unwrap();
        if let Msg::ClusterStatus(ClusterStatus{connected, ..}) = envelope.msg {
            if connected.len() == 2 {
                println!("{:#?}", connected);
                println!("Cluster connected in {} ms",
                         (SteadyTime::now() - start).num_milliseconds());
                break;
            }
        }
    }
}

fn create_replica_pids(nodes: &Vec<CrNode>) -> Vec<Pid> {
    ["replica1", "replica2", "replica3"].iter().zip(nodes).map(|(name, node)| {
        Pid {
            name: name.to_string(),
            group: None,
            node: node.id.clone()
        }
    }).collect()
}

fn create_node_ids() -> Vec<NodeId> {
    (1..4).map(|n| {
        NodeId {
            name: format!("node{}", n),
            addr: format!("127.0.0.1:1100{}", n)
        }
    }).collect()
}
