#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

extern crate orset;
extern crate rmp_serde as msgpack;
extern crate protobuf;
extern crate amy;
extern crate time;
extern crate net2;
extern crate libc;
extern crate ferris;
extern crate hdrsample;
extern crate chashmap;
extern crate coco;
extern crate parking_lot;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;

extern crate serde;
extern crate serde_bytes;

#[macro_use]
extern crate serde_derive;

#[macro_use]
mod metrics;

mod node_id;
mod node;
mod members;
mod pid;
mod process;
mod envelope;
mod cluster;
mod msg;
mod timer_wheel;
mod service;
mod correlation_id;
mod histogram;
mod processes;
mod scheduler;

pub mod serialize;

pub mod errors;

pub use errors::Result;
pub use node_id::NodeId;
pub use node::Node;
pub use pid::Pid;
pub use process::Process;
pub use envelope::Envelope;
pub use correlation_id::CorrelationId;
pub use msg::Msg;
pub use metrics::Metric;
pub use processes::Processes;

pub use cluster::{
    ClusterServer,
    ClusterStatus,
};

pub use service::{
    Service,
    ConnectionHandler,
    ConnectionMsg,
    ServiceHandler,
    TcpServerHandler,
};

use std::thread::{self, JoinHandle};
use std::sync::mpsc::channel;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use amy::Poller;
use slog::DrainExt;
use cluster::ClusterMsg;
use scheduler::Scheduler;

const TIMEOUT: usize = 5000; // ms

/// Start a node in the rabble cluster and return it along with the handles to all threads started
/// by rabble.
///
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<'de, T>(node_id: NodeId, logger: Option<slog::Logger>) -> (Node<T>, Vec<JoinHandle<()>>)
  where T: Serialize + Deserialize<'de> + Send + 'static + Clone + Debug + Sync,
{
    let logger = match logger {
        Some(logger) => logger.new(o!("node_id" => node_id.to_string())),
        None => slog::Logger::root(slog_stdlog::StdLog.fuse(), o!("node_id" => node_id.to_string()))
    };

    let processes = Processes::new();

    // Just launch a single scheduler for now.
    // TODO: Make this configurable.
    let scheduler_pid = Pid { name: "scheduler1".to_owned(), group: None, node: node_id.clone() };
    let scheduler = Scheduler::new(scheduler_pid, processes.clone());

    let h0 = thread::Builder::new().name(format!("scheduler1::{}", node_id)).spawn(move || {
        scheduler.run()
    }).unwrap();


    let mut poller = Poller::new().unwrap();
    let (cluster_tx, cluster_rx) = channel();
    let cluster_server = ClusterServer::new(node_id.clone(),
                                            processes.clone(),
                                            cluster_rx,
                                            poller.get_registrar().unwrap(),
                                            logger.clone());

    let h1 = thread::Builder::new().name(format!("cluster_server::{}", node_id)).spawn(move || {
        cluster_server.run()
    }).unwrap();

    let _cluster_tx = cluster_tx.clone();

    // TODO: Poll in the cluster server thread. There is no reason to have a separate thread here.
    let h2 = thread::Builder::new().name(format!("poller::{}", node_id)).spawn(move || {
        loop {
            let notifications = poller.wait(TIMEOUT).unwrap();
            if let Err(_) = _cluster_tx.send(ClusterMsg::PollNotifications(notifications)) {
                // The process is exiting
                return;
            }
        }
    }).unwrap();

    (Node::new(node_id, processes, cluster_tx, logger), vec![h0, h1, h2])
}
