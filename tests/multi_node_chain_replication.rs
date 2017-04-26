extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;

extern crate slog;
extern crate slog_stdlog;
extern crate slog_envlogger;
extern crate slog_term;
extern crate log;
extern crate time;

mod utils;

use std::{str, thread};
use std::net::TcpStream;
use amy::{Poller, Receiver, Sender};
use time::{SteadyTime, Duration};

use utils::messages::*;
use utils::replica::Replica;
use utils::api_server;
use utils::{
    wait_for,
    start_nodes,
    send,
    test_pid,
    register_test_as_service
};

use rabble::{
    Pid,
    Envelope,
    Msg,
    ClusterStatus,
    MsgpackSerializer,
    Serialize,
    Node,
    CorrelationId
};

const API_SERVER_IP: &'static str = "127.0.0.1:12001";
const NUM_NODES: usize = 3;

type CrNode = Node<RabbleUserMsg>;
type CrReceiver = Receiver<Envelope<RabbleUserMsg>>;
type CrSender = Sender<Envelope<RabbleUserMsg>>;

#[test]
fn chain_replication() {
    let (nodes, mut handles) = start_nodes(NUM_NODES);

    // We create an amy channel so that we can pretend this test is a service.
    // We register the sender with all nodes so that we can check the responses to admin calls
    // like node.get_cluster_status().
    let mut poller = Poller::new().unwrap();
    let (test_tx, test_rx) = poller.get_registrar().unwrap().channel().unwrap();

    register_test_as_service(&mut poller, &nodes, &test_tx, &test_rx);

    let pids = create_replica_pids(&nodes);

    // We only send API requests to node1, so only bother starting an API server on this node
    let (service_pid, service_tx, service_handle) = api_server::start(nodes[0].clone());
    handles.push(service_handle);

    spawn_replicas(&nodes, &pids);

    join_nodes(&nodes, &mut poller, &test_rx);

    run_client_operations(&pids[0]);

    verify_histories(&pids);

    shutdown(nodes, service_pid, service_tx);

    for h in handles {
        h.join().unwrap();
    }
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

fn join_nodes(nodes: &Vec<CrNode>, poller: &mut Poller, test_rx: &CrReceiver) {
    nodes[0].join(&nodes[1].id).unwrap();
    nodes[0].join(&nodes[2].id).unwrap();
    wait_for_connected_cluster(&nodes, poller, test_rx);
}

/// launch 3 clients and send concurrent operations to the head of the chain
fn run_client_operations(pid: &Pid) {
    let mut client_handles = Vec::new();
    for i in 0..3 {
        let pid = pid.clone();
        let h = thread::spawn(move || {
            let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
            let mut serializer = MsgpackSerializer::new();
            sock.set_nonblocking(true).unwrap();
            send(&mut sock, &mut serializer, ApiClientMsg::Op(pid, i));
            assert_eq!(true, wait_for(Duration::seconds(5), || {
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
            service_pid: Pid,
            service_tx: CrSender)
{
    let envelope = Envelope::new(service_pid, test_pid(nodes[0].id.clone()), Msg::Shutdown, None);
    service_tx.send(envelope).unwrap();
    for node in nodes {
        node.shutdown();
    }
}

fn wait_for_connected_cluster(nodes: &Vec<CrNode>,
                              poller: &mut Poller,
                              test_rx: &CrReceiver) {
    let start = SteadyTime::now();
    let mut stable_count = 0;
    while stable_count < nodes.len() {
        stable_count = 0;
        for node in nodes {
            let correlation_id = CorrelationId::pid(test_pid(node.id.clone()));
            node.cluster_status(correlation_id).unwrap();
            // We are only polling on the test channel, so we don't need to know what woke the poller
            let notifications = poller.wait(5000).unwrap();
            assert_eq!(1, notifications.len());
            let envelope = test_rx.try_recv().unwrap();
            if let Msg::ClusterStatus(ClusterStatus{established,
                                                    num_connections, ..}) = envelope.msg
            {
                // Ensure that we are in a stable state. We have 2 established connections and no
                // non-established connections that may cause established ones to disconnect.
                if established.len() == 2  && num_connections == 2 {
                    println!("Cluster connected in {} ms at {}",
                             (SteadyTime::now() - start).num_milliseconds(), node.id);
                    stable_count +=1 ;
                }
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
