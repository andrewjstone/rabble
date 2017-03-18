use std::thread::{self, JoinHandle};
use std::net::TcpStream;
use amy::{Poller, Receiver, Sender};
use slog::{self, DrainExt};
use slog_term;
use slog_envlogger;
use slog_stdlog;
use time::{SteadyTime, Duration};
use utils::messages::*;
use rabble::{
    self,
    NodeId,
    Node,
    MsgpackSerializer,
    Serialize,
    Envelope,
    Pid,
    CorrelationId,
    Msg,
    Req,
    Rpy,
    Metric
};

type CrNode = Node<RabbleUserMsg>;
type CrReceiver = Receiver<Envelope<RabbleUserMsg>>;
type CrSender = Sender<Envelope<RabbleUserMsg>>;

/// Wait for a function to return true
///
/// After each call of `f()` that returns `false`, sleep for `sleep_time`
/// Returns true if `f()` returns true before the timeout expires
/// Returns false if the runtime of the test exceeds `timeout`
#[allow(dead_code)] // Not used in all tests
pub fn wait_for<F>(timeout: Duration, mut f: F) -> bool
    where F: FnMut() -> bool
{
    let sleep_time = Duration::milliseconds(10);
    let start = SteadyTime::now();
    while let false = f() {
        thread::sleep(sleep_time.to_std().unwrap());
        if SteadyTime::now() - start > timeout {
            return false;
        }
    }
    true
}

/// Send a message over a non-blocking socket
/// Wait for it to finish sending or timeout after 5 seconds
/// In practice the first call to serializer.write_msgs should succeed unless the TCP send buffer is
/// tiny.
#[allow(dead_code)] // Not used in all tests
pub fn send(sock: &mut TcpStream,
            serializer: &mut MsgpackSerializer<ApiClientMsg>,
            msg: ApiClientMsg)
{
    if let Ok(true) = serializer.write_msgs(sock, Some(&msg)) {
        return;
    }
    // Just busy wait instead of using a poller in this test.
    assert_eq!(true, wait_for(Duration::seconds(5), || {
        // We don't know if it's writable, but we want to actually try the write
        serializer.set_writable();
        match serializer.write_msgs(sock, None) {
            Ok(true) => true,
            Ok(false) => false,
            Err(e) => {
                println!("Failed to write to socket: {}", e);
                assert!(false);
                unreachable!();
            }
        }
    }));
}


#[allow(dead_code)] // Not used in all tests
pub fn create_node_ids(n: usize) -> Vec<NodeId> {
    (1..n + 1).map(|n| {
        NodeId {
            name: format!("node{}", n),
            addr: format!("127.0.0.1:1100{}", n)
        }
    }).collect()
}

#[allow(dead_code)] // Not used in all tests
pub fn start_nodes(n: usize) -> (Vec<Node<RabbleUserMsg>>, Vec<JoinHandle<()>>) {
    let term = slog_term::streamer().build();
    let drain = slog_envlogger::LogBuilder::new(term)
        .filter(None, slog::FilterLevel::Debug).build();
    let root_logger = slog::Logger::root(drain.fuse(), None);
    slog_stdlog::set_logger(root_logger.clone()).unwrap();
    create_node_ids(n).into_iter().fold((Vec::new(), Vec::new()),
                                  |(mut nodes, mut handles), node_id| {
        let (node, handle_list) = rabble::rouse(node_id, Some(root_logger.clone()));
        nodes.push(node);
        handles.extend(handle_list);
        (nodes, handles)
    })
}

#[allow(dead_code)] // Not used in all tests
pub fn test_pid(node_id: NodeId) -> Pid {
    Pid {
        name: "test-runner".to_string(),
        group: None,
        node: node_id
    }
}

#[allow(dead_code)] // Not used in all tests
pub fn register_test_as_service(poller: &mut Poller,
                                nodes: &Vec<CrNode>,
                                test_tx: &CrSender,
                                test_rx: &CrReceiver)
{
    for node in nodes {
        let test_pid = test_pid(node.id.clone());
        let correlation_id = CorrelationId::pid(test_pid.clone());
        node.register_service(&test_pid, &test_tx).unwrap();
        // Wait for registration to succeed
        loop {
            node.send(Envelope {
                to: cluster_server_pid(node.id.clone()),
                from: test_pid.clone(),
                msg: Msg::Req(Req::GetMetrics),
                correlation_id: correlation_id.clone(),
            }).unwrap();
            let notifications = poller.wait(10).unwrap();
            if notifications.len() != 0 {
                // We have registered, otherwise we wouldn't have gotten a response
                // Let's drain the receiver, because we may have returned from a previous poll
                // before the previous ClusterStatus response was sent
                while let Ok(envelope) = test_rx.try_recv() {
                    assert_matches!(envelope.msg, Msg::Rpy(Rpy::Metrics(_)));
                }
                break;
            }
        }
    }

}

#[allow(dead_code)]
pub fn cluster_server_pid(node_id: NodeId) -> Pid {
    Pid {
        group: Some("rabble".to_string()),
        name: "cluster_server".to_string(),
        node: node_id
    }
}

#[allow(dead_code)]
pub fn connections_stable(n: i64, metrics: Vec<(String, Metric)>) -> bool {
    // Ensure that we are in a stable state. We have 2 established connections and no
    // non-established connections that may cause established ones to disconnect.
    let num_connections = {
        if let Metric::Gauge(n) = metrics.iter()
            .find(|&&(ref name, _)| name == "connections").unwrap().1
        {
            n
        } else {
            unreachable!();
        }
    };

    let num_established = {
        if let Metric::Gauge(n) = metrics.iter()
            .find(|&&(ref name, _)| name == "established_connections").unwrap().1
        {
            n
        } else {
            unreachable!();
        }
    };

    num_connections == n && num_established == n
}
