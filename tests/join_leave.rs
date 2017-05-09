//! Test cluster joining and leaving

extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
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

use std::str;
use amy::{Poller, Receiver};
use time::Duration;

use utils::messages::*;
use utils::{
    wait_for,
    start_nodes,
    test_pid,
    register_test_as_service
};

use rabble::{
    Envelope,
    Msg,
    ClusterStatus,
    Node,
    CorrelationId
};

const NUM_NODES: usize = 3;

#[test]
fn join_leave() {
    let (nodes, handles) = start_nodes(NUM_NODES);

    // We create an amy channel so that we can pretend this test is a service.
    // We register the sender with all nodes so that we can check the responses to admin calls
    // like node.get_cluster_status().
    let mut poller = Poller::new().unwrap();
    let (test_tx, test_rx) = poller.get_registrar().unwrap().channel().unwrap();

    register_test_as_service(&mut poller, &nodes, &test_tx, &test_rx);

    // join node1 to node2
    // Wait for the cluster status of both nodes to show they are connected
    nodes[0].join(&nodes[1].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], &test_rx, 1));
    assert!(wait_for_cluster_status(&nodes[1], &test_rx, 1));

    // Join node1 to node3. This will cause a delta to be sent from node1 to node2. Node3 will also
    // connect to node2 and send it's members, since it will learn of node2 from node1. Either way
    // all nodes should stabilize as knowing about each other.
    nodes[0].join(&nodes[2].id).unwrap();
    for node in &nodes {
        assert!(wait_for_cluster_status(&node, &test_rx, 2));
    }

    // Remove node2 from the cluster. This will cause a delta of the remove to be broadcast to node1
    // 1 and node3. Note that the request is sent to node1, not the node that is leaving.
    nodes[0].leave(&nodes[1].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], &test_rx, 1));
    assert!(wait_for_cluster_status(&nodes[2], &test_rx, 1));


    // Remove node1 from the cluster. This request goest to node1. It's possible in production that
    // the broadcast doesn't make it to node3 before node1 disconnects from node3 due to the
    // membership check on the next tick that removes connections.
    // TODO: make that work
    nodes[0].leave(&nodes[0].id).unwrap();
    assert!(wait_for_cluster_status(&nodes[0], &test_rx, 0));
    assert!(wait_for_cluster_status(&nodes[2], &test_rx, 0));

    for node in nodes {
        node.shutdown();
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn wait_for_cluster_status(node: &Node<RabbleUserMsg>,
                           test_rx: &Receiver<Envelope<RabbleUserMsg>>,
                           num_connected: usize) -> bool
{
    let timeout = Duration::seconds(5);
    let test_pid = test_pid(node.id.clone());
    wait_for(timeout, || {
        let correlation_id = CorrelationId::pid(test_pid.clone());
        node.cluster_status(correlation_id.clone()).unwrap();
        if let Ok(envelope) = test_rx.try_recv() {
            if let Msg::ClusterStatus(ClusterStatus{established, num_connections, ..})
                = envelope.msg
            {
                if established.len() == num_connected  && num_connections == num_connected {
                    return true;
                }
            }
        }
        false
    })
}

