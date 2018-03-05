#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

extern crate orset;
extern crate rmp_serde as msgpack;
extern crate amy;
extern crate time;
extern crate net2;
extern crate libc;
extern crate ferris;
extern crate hdrhistogram;
extern crate futures;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;

extern crate serde;
extern crate serde_bytes;

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
mod terminal;

pub mod histogram;
pub mod channel;
pub mod errors;

pub use errors::Result;
pub use node_id::NodeId;
pub use node::Node;
pub use pid::Pid;
pub use process::Process;
pub use envelope::Envelope;
pub use msg::Msg;
pub use terminal::{Terminal, TimerId};

pub use cluster::{
    ClusterServer,
    ClusterStatus,
};

pub use executor::{
    Executor,
    ExecutorStatus,
    ExecutorMetrics
};

use std::thread::{self, JoinHandle};
use std::sync::mpsc::channel;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use slog::DrainExt;

/// Start a node in the rabble cluster and return it along with a handle to the thread of the
/// cluster server.
///
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<'de, T>(node_id: NodeId, logger: Option<slog::Logger>) -> (Node<T>, JoinHandle<()>)
  where T: Serialize + Deserialize<'de> + Clone + Debug + Send + 'static,
{
    let logger = match logger {
        Some(logger) => logger.new(o!("node_id" => node_id.to_string())),
        None => slog::Logger::root(slog_stdlog::StdLog.fuse(), o!("node_id" => node_id.to_string()))
    };

    let cluster_server = ClusterServer::new(node_id.clone(), logger.clone());
    let cluster_tx = cluster_server.sender();

    let h = thread::Builder::new().name(format!("cluster_server::{}", node_id)).spawn(move || {
        cluster_server.run()
    }).unwrap();

    (Node::new(node_id, cluster_tx, logger), h)
}
