//! Test cluster joining and leaving

extern crate amy;
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

use time::Duration;

use utils::messages::*;
use utils::{
    wait_for,
    start_nodes,
};

use rabble::{
    ClusterStatus,
    Node,
    channel
};

const NUM_NODES: usize = 3;

#[test]
fn join_leave() {
    let (nodes, handles) = start_nodes(NUM_NODES);

    // join node1 to node2
    // Wait for the cluster status of both nodes to show they are connected
    nodes[0].join(&nodes[1].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], 1));
    assert!(wait_for_cluster_status(&nodes[1], 1));

    // Join node1 to node3. This will cause a delta to be sent from node1 to node2. Node3 will also
    // connect to node2 and send it's members, since it will learn of node2 from node1. Either way
    // all nodes should stabilize as knowing about each other.
    nodes[0].join(&nodes[2].id).unwrap();
    for node in &nodes {
        assert!(wait_for_cluster_status(&node, 2));
    }

    // Remove node2 from the cluster. This will cause a delta of the remove to be broadcast to node1
    // 1 and node3. Note that the request is sent to node1, not the node that is leaving.
    nodes[0].leave(&nodes[1].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], 1));
    assert!(wait_for_cluster_status(&nodes[2], 1));


    // Remove node1 from the cluster. This request goest to node1. It's possible in production that
    // the broadcast doesn't make it to node3 before node1 disconnects from node3 due to the
    // membership check on the next tick that removes connections.
    // TODO: make that work
    nodes[0].leave(&nodes[0].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], 0));
    assert!(wait_for_cluster_status(&nodes[2], 0));

    for node in nodes {
        node.shutdown();
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn wait_for_cluster_status(node: &Node<TestMsg>,
                           num_connected: usize) -> bool
{
    let timeout = Duration::seconds(5);
    wait_for(timeout, || {
        let (tx, rx) = std::sync::mpsc::channel::<ClusterStatus>();
        let tx = Box::new(tx) as Box<channel::Sender<ClusterStatus>>;
        node.cluster_status(tx).unwrap();
        if let Ok(ClusterStatus{established, num_connections, ..}) = rx.recv() {
            if established.len() == num_connected  && num_connections == num_connected {
                return true;
            }
        }
        false
    })
}
