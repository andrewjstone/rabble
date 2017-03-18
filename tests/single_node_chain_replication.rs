extern crate amy;
extern crate rabble;
extern crate slog;
extern crate slog_term;
extern crate slog_envlogger;
extern crate slog_stdlog;
extern crate time;
#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;
extern crate protobuf;

mod utils;

use std::thread;
use std::net::TcpStream;
use std::str;
use amy::Sender;

use utils::messages::*;
use utils::replica::Replica;
use utils::api_server;

use rabble::{
    Pid,
    NodeId,
    Envelope,
    Msg,
    Req,
    MsgpackSerializer,
    Serialize,
    Node,
    CorrelationId
};

const CLUSTER_SERVER_IP: &'static str = "127.0.0.1:11001";
const API_SERVER_IP: &'static str  = "127.0.0.1:12001";

#[test]
fn chain_replication() {
    let node_id = NodeId {name: "node1".to_string(), addr: CLUSTER_SERVER_IP.to_string()};
    let test_pid = Pid { name: "test-runner".to_string(), group: None, node: node_id.clone()};
    let (node, mut handles) = rabble::rouse::<RabbleUserMsg>(node_id, None);

    let pids = create_replica_pids(&node.id);

    let (service_pid, service_tx, service_handle) = api_server::start(node.clone());
    handles.push(service_handle);

    spawn_replicas(&node, &pids);

    run_client_operations(&pids);

    verify_histories(&pids);

    shutdown(node, test_pid, service_pid, service_tx);

    for h in handles {
        h.join().unwrap();
    }

}

fn shutdown(node: Node<RabbleUserMsg>,
            test_pid: Pid,
            service_pid: Pid,
            service_tx: Sender<Envelope<RabbleUserMsg>>)
{
    let shutdown_envelope = Envelope {
        to: service_pid,
        from: test_pid.clone(),
        msg: Msg::Req(Req::Shutdown),
        correlation_id: CorrelationId::pid(test_pid)
    };
    service_tx.send(shutdown_envelope).unwrap();
    node.shutdown();

}

fn create_replica_pids(node_id: &NodeId) -> Vec<Pid> {
    ["replica1", "replica2", "replica3"].iter().map(|name| {
        Pid {
            name: name.to_string(),
            group: None,
            node: node_id.clone()
        }
    }).collect()
}

fn spawn_replicas(node: &Node<RabbleUserMsg>, pids: &Vec<Pid>) {
    // Launch the three replicas participating in chain replication
    for i in 0..pids.len() {
        let next = if i == pids.len() - 1 {
            None
        } else {
            Some(pids[i + 1].clone())
        };

        let replica = Box::new(Replica::new(pids[i].clone(), next));
        node.spawn(&pids[i], replica).unwrap();
    }
}

/// launch 3 clients and send concurrent operations to the head of the chain
fn run_client_operations(pids: &Vec<Pid>) {
    let mut client_handles = Vec::new();
    for i in 0..3 {
        let pids = pids.clone();
        let h = thread::spawn(move || {
            let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
            let mut serializer = MsgpackSerializer::new();
            assert_matches!(serializer.write_msgs(&mut sock,
                                                  Some(&ApiClientMsg::Op(pids[0].clone(), i))),
                            Ok(true));
            sock.set_nonblocking(true).unwrap();
            loop {
                thread::sleep(time::Duration::milliseconds(10).to_std().unwrap());
                match serializer.read_msg(&mut sock) {
                    Ok(None) => (),
                    Ok(Some(reply)) => {
                        assert_eq!(ApiClientMsg::OpComplete, reply);
                        break;
                    },
                    Err(e) => {
                        println!("{}", e);
                        assert!(false)
                    }
                }
            }
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
                thread::sleep(time::Duration::milliseconds(10).to_std().unwrap());
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
