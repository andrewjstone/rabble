use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use envelope::Envelope;
use process::Process;
use pid::Pid;

pub enum ExecutorMsg<T: Encodable + Decodable> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    User(Envelope<T>)
}
