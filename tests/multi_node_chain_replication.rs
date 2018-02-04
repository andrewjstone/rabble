extern crate rabble;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate slog;
extern crate slog_stdlog;
extern crate slog_envlogger;
extern crate slog_term;
extern crate log;
extern crate time;

mod utils;

use std::sync::mpsc;
use time::SteadyTime;

use utils::messages::*;
use utils::replica::Replica;
use utils::{
    chain_repl,
    start_nodes,
    test_pid,
};

use rabble::{
    Pid,
    Envelope,
    ClusterStatus,
    Node,
    channel
};

type CrNode = Node<TestMsg>;


#[test]
fn chain_replication() {
    let (nodes, handles) = start_nodes(3);

    let test_pid = test_pid(nodes[0].id.clone());
    let (test_tx, test_rx) = mpsc::channel();
    let test_tx = Box::new(test_tx) as Box<channel::Sender<Envelope<TestMsg>>>;
    // Send all requests from node 1
    nodes[0].register_service(&test_pid, test_tx).unwrap();

    let pids = create_replica_pids(&nodes);
    spawn_replicas(&nodes, &pids);
    join_nodes(&nodes);
    chain_repl::run_client_operations(&nodes[0], &pids[0], &test_pid, &test_rx);
    chain_repl::verify_histories(&nodes[0], &pids, &test_pid, &test_rx);

    for node in nodes {
        node.shutdown();
    }

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

fn join_nodes(nodes: &Vec<CrNode>) {
    nodes[0].join(&nodes[1].id).unwrap();
    nodes[0].join(&nodes[2].id).unwrap();
    wait_for_connected_cluster(&nodes);
}

/// Ensure evey node is connected to every other node
fn wait_for_connected_cluster(nodes: &Vec<CrNode>) {
    let start = SteadyTime::now();
    let mut stable_count = 0;
    while stable_count < nodes.len() {
        stable_count = 0;
        for node in nodes {
            let (tx, rx) = mpsc::channel();
            let tx = Box::new(tx) as Box<channel::Sender<ClusterStatus>>;
            node.cluster_status(tx).unwrap();
            match rx.recv() {
                Ok(ClusterStatus{established, num_connections, ..}) => {
                    // Ensure that we are in a stable state. We have 2 established connections and no
                    // non-established connections that may cause established ones to disconnect.
                    if established.len() == 2  && num_connections == 2 {
                        println!("Cluster connected in {} ms at {}",
                                 (SteadyTime::now() - start).num_milliseconds(), node.id);
                        stable_count +=1 ;
                    }
                }
                e => {
                    println!("Failed to retrieve cluster status: {:?}", e);
                    assert!(false)
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
