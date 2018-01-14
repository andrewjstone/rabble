#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

extern crate orset;
extern crate rmp_serde as msgpack;
extern crate protobuf;
extern crate amy;
extern crate time;
extern crate net2;
extern crate libc;
extern crate ferris;
//extern crate hdrsample;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;

extern crate serde;

#[macro_use]
extern crate serde_derive;

mod node_id;
mod node;
mod members;
mod pid;
mod process;
mod envelope;
mod executor;
mod cluster;
mod msg;
mod timer_wheel;
mod service;
mod correlation_id;
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

pub use cluster::{
    ClusterServer,
    ClusterStatus,
};

pub use executor::{
    Executor,
    ExecutorStatus,
    ExecutorMetrics
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

const TIMEOUT: usize = 5000; // ms

/// Start a node in the rabble cluster and return it along with the handles to all threads started
/// by rabble.
///
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<'de, T>(node_id: NodeId, logger: Option<slog::Logger>) -> (Node<T>, Vec<JoinHandle<()>>)
  where T: Serialize + Deserialize<'de> + Send + 'static + Clone + Debug,
{
    let logger = match logger {
        Some(logger) => logger.new(o!("node_id" => node_id.to_string())),
        None => slog::Logger::root(slog_stdlog::StdLog.fuse(), o!("node_id" => node_id.to_string()))
    };

    let mut poller = Poller::new().unwrap();
    let (exec_tx, exec_rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let cluster_server = ClusterServer::new(node_id.clone(),
                                            cluster_rx,
                                            exec_tx.clone(),
                                            poller.get_registrar().unwrap(),
                                            logger.clone());
    let executor = Executor::new(node_id.clone(),
                                 exec_tx.clone(),
                                 exec_rx,
                                 cluster_tx.clone(),
                                 logger.clone());

    let h1 = thread::Builder::new().name(format!("cluster_server::{}", node_id)).spawn(move || {
        cluster_server.run()
    }).unwrap();

    let h2 = thread::Builder::new().name(format!("executor::{}", node_id)).spawn(move || {
        executor.run()
    }).unwrap();

    let _cluster_tx = cluster_tx.clone();
    let h3 = thread::Builder::new().name(format!("poller::{}", node_id)).spawn(move || {
        loop {
            let notifications = poller.wait(TIMEOUT).unwrap();
            if let Err(_) = _cluster_tx.send(ClusterMsg::PollNotifications(notifications)) {
                // The process is exiting
                return;
            }
        }
    }).unwrap();

    (Node::new(node_id, exec_tx, cluster_tx, logger), vec![h1, h2, h3])
}
