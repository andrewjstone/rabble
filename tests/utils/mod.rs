extern crate time;
extern crate slog;
extern crate slog_term;
extern crate slog_envlogger;
extern crate slog_stdlog;

pub mod replica;
pub mod api_server;
pub mod messages;

use std::thread::{self, JoinHandle};
use self::slog::DrainExt;
use self::time::{SteadyTime, Duration};
use utils::messages::*;
use rabble::{
    self,
    NodeId,
    Node
};

/// Wait for a function to return true
///
/// After each call of `f()` that returns `false`, sleep for `sleep_time`
/// Returns true if `f()` returns true before the timeout expires
/// Returns false if the runtime of the test exceeds `timeout`
#[allow(dead_code)] // Not used in all tests
pub fn wait_for<F>(sleep_time: Duration, timeout: Duration, mut f: F) -> bool
    where F: FnMut() -> bool
{
    let start = SteadyTime::now();
    while let false = f() {
        thread::sleep(sleep_time.to_std().unwrap());
        if SteadyTime::now() - start > timeout {
            return false;
        }
    }
    true
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
    let root_logger = slog::Logger::root(drain.fuse(), vec![]);
    slog_stdlog::set_logger(root_logger.clone()).unwrap();
    create_node_ids(n).into_iter().fold((Vec::new(), Vec::new()),
                                  |(mut nodes, mut handles), node_id| {
        let (node, handle_list) = rabble::rouse(node_id, Some(root_logger.clone()));
        nodes.push(node);
        handles.extend(handle_list);
        (nodes, handles)
    })
}

