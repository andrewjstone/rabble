extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;

mod utils;

use std::{thread, time};
use std::thread::JoinHandle;
use std::net::TcpStream;
use std::str;
use amy::{Poller, Receiver};

use utils::messages::*;
use utils::replica::Replica;
use utils::api_server;

use rabble::{
    Pid,
    NodeId,
    SystemEnvelope,
    SystemMsg,
    ClusterStatus,
    MsgpackSerializer,
    Serialize,
    Node,
    CorrelationId
};

const API_SERVER_IP: &'static str = "127.0.0.1:12001";
const ADMIN_SERVER_IP: &'static str = "127.0.0.1:12002";

type CrNode = Node<ProcessMsg, SystemUserMsg>;
type CrReceiver = Receiver<SystemEnvelope<SystemUserMsg>>;

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

    for h in handles {
        h.join().unwrap();
    }
}

fn start_nodes() -> (Vec<CrNode>, Vec<JoinHandle<()>>) {
    create_node_ids().into_iter().fold((Vec::new(), Vec::new()),
                                  |(mut nodes, mut handles), node_id| {
        let (node, handle_list) = rabble::rouse(node_id);
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

fn wait_for_connected_cluster(node: &CrNode,
                              poller: &mut Poller,
                              correlation_id: CorrelationId,
                              test_rx: &CrReceiver) {
    let mut count = 0;
    loop {
        count += 1;
        thread::sleep(time::Duration::from_millis(10));
        node.cluster_status(correlation_id.clone()).unwrap();
        // We are only polling on the test channel, so we don't need to know what woke the poller
        let _ = poller.wait(5000).unwrap();
        let envelope = test_rx.try_recv().unwrap();
        if let SystemMsg::ClusterStatus(ClusterStatus{connected, ..}) = envelope.msg {
            println!("{:#?}", connected);
            if connected.len() == 2 {
                println!("Cluster connected in > {} ms", 10*count);
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
