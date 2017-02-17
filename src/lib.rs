#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

extern crate orset;
extern crate rustc_serialize;
extern crate rmp_serialize as msgpack;
extern crate protobuf;
extern crate amy;
extern crate time;
extern crate net2;
extern crate libc;
extern crate ferris;
extern crate hdrsample;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;

mod node_id;
mod node;
mod members;
mod pid;
mod process;
mod envelope;
mod executor;
mod msg;
mod executor_msg;
mod cluster_msg;
mod external_msg;
mod cluster_server;
mod timer_wheel;
mod service;
mod service_handler;
mod correlation_id;
mod thread_handler;
mod tcp_server_handler;
mod connection_handler;
mod serialize;
mod msgpack_serializer;
mod protobuf_serializer;
mod status;
mod histogram;

pub mod errors;

pub use histogram::Histogram;
pub use status::{StatusVal, StatusTable};
pub use errors::Result;
pub use node_id::NodeId;
pub use node::Node;
pub use pid::Pid;
pub use process::Process;
pub use envelope::Envelope;
pub use service::Service;
pub use correlation_id::CorrelationId;
pub use msg::Msg;

pub use thread_handler::ThreadHandler;
pub use connection_handler::{
    ConnectionHandler,
    ConnectionMsg
};
pub use serialize::Serialize;
pub use msgpack_serializer::MsgpackSerializer;
pub use protobuf_serializer::ProtobufSerializer;
pub use tcp_server_handler::TcpServerHandler;
pub use service_handler::ServiceHandler;

use std::thread::{self, JoinHandle};
use std::sync::mpsc::channel;
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use amy::Poller;
use cluster_msg::ClusterMsg;
use cluster_server::ClusterServer;
use executor::Executor;

use slog::DrainExt;

const TIMEOUT: usize = 5000; // ms

/// Start a node in the rabble cluster and return it along with the handles to all threads started
/// by rabble.
///
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<T>(node_id: NodeId, logger: Option<slog::Logger>) -> (Node<T>, Vec<JoinHandle<()>>)
  where T: Encodable + Decodable + Send + 'static + Clone + Debug,
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
                                            poller.get_registrar(),
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

