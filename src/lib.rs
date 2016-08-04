extern crate orset;
extern crate rustc_serialize;
extern crate rmp_serialize as msgpack;
extern crate amy;
extern crate time;
extern crate net2;

mod node_id;
mod node;
mod members;
mod pid;
mod process;
mod envelope;
mod executor;
mod internal_msg;
mod external_msg;
mod poller;
mod cluster_server;
mod timer_wheel;

pub use node_id::NodeId;
pub use node::Node;
pub use pid::Pid;
pub use process::Process;
pub use envelope::Envelope;

use std::sync::mpsc::channel;
use rustc_serialize::{Encodable, Decodable};
use amy::Poller;
use internal_msg::InternalMsg;

const TIMEOUT: usize = 5000; // ms

/// Start a node in the rabble cluster
/// All nodes in a cluster must be parameterized by the same type.
pub fn rouse<T: Encodable + Decodable>(node_id: NodeId) -> Node {
    Node {
        id: node_id
    }
}

