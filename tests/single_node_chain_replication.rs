extern crate rabble;
extern crate serde;
#[macro_use]
extern crate serde_derive;


mod utils;

use std::sync::mpsc;

use utils::messages::*;
use utils::replica::Replica;
use utils::{
    chain_repl,
    test_pid,
    start_nodes
};

use rabble::{
    Pid,
    NodeId,
    Envelope,
    Node,
    channel
};

#[test]
fn chain_replication() {
    let (nodes, handles) = start_nodes(1);
    let node = nodes[0].clone();
    let test_pid = test_pid(node.id.clone());

    let (test_tx, test_rx) = mpsc::channel();
    let test_tx = Box::new(test_tx) as Box<channel::Sender<Envelope<TestMsg>>>;
    node.register_service(&test_pid, test_tx).unwrap();

    let pids = create_replica_pids(&node.id);
    spawn_replicas(&node, &pids);
    chain_repl::run_client_operations(&node, &pids[0], &test_pid, &test_rx);
    chain_repl::verify_histories(&node, &pids, &test_pid, &test_rx);

    node.shutdown();

    for h in handles {
        h.join().unwrap();
    }

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

fn spawn_replicas(node: &Node<TestMsg>, pids: &Vec<Pid>) {
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

