use std::fmt::Debug;
use serde::{Serialize, Deserialize};
use pid::Pid;
use msg::Msg;

/// Envelopes are routable to processes on all nodes and threads running on the same node as this
/// process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub to: Pid,
    pub from: Pid,
    pub msg: Msg<T>
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone> Envelope<T> {
    pub fn new(to: Pid, from: Pid, msg: Msg<T>) -> Envelope<T> {
        Envelope {
            to: to,
            from: from,
            msg: msg
        }
    }
}
