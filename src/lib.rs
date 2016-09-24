#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

extern crate orset;
extern crate rustc_serialize;
extern crate rmp_serialize as msgpack;
extern crate amy;
extern crate time;
extern crate net2;

mod errors;
mod node_id;
mod node;
mod members;
mod pid;
mod process;
mod envelope;
mod executor;
mod executor_msg;
mod cluster_msg;
mod external_msg;
mod cluster_server;
mod timer_wheel;
mod executor_status;
mod cluster_status;
mod system_msg;
mod service;
mod handler;
mod correlation_id;
mod system_envelope_handler;
mod tcp_server_handler;
mod connection;
mod protocol;
mod msgpack_protocol;

pub use node_id::NodeId;
pub use node::Node;
pub use pid::Pid;
pub use process::Process;
pub use envelope::Envelope;
pub use service::Service;
pub use correlation_id::CorrelationId;
pub use system_msg::SystemMsg;
pub use system_envelope_handler::SystemEnvelopeHandler;
pub use connection::{
    Connection,
    ConnectionMsg
};
pub use protocol::Protocol;

pub use msgpack_protocol::MsgpackProtocol;

use std::thread::{self, JoinHandle};
use std::sync::mpsc::channel;
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use amy::Poller;
use cluster_msg::ClusterMsg;
use cluster_server::ClusterServer;
use executor::Executor;

const TIMEOUT: usize = 5000; // ms

/// Start a node in the rabble cluster and return it along with the handles to all threads started
/// by rabble.
///
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<T, U>(node_id: NodeId) -> (Node<T, U>, Vec<JoinHandle<()>>)
  where T: Encodable + Decodable + Send + 'static,
        U: Debug + Clone + Send + 'static {
    let mut poller = Poller::new().unwrap();
    let (exec_tx, exec_rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let cluster_server = ClusterServer::new(node_id.clone(),
                                            cluster_rx,
                                            exec_tx.clone(),
                                            poller.get_registrar());
    let executor = Executor::new(node_id.clone(), exec_tx.clone(), exec_rx, cluster_tx.clone());

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

    (Node::new(node_id, exec_tx, cluster_tx), vec![h1, h2, h3])
}

