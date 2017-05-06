use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use msg::Msg;
use envelope::Envelope;
use correlation_id::CorrelationId;

pub trait Process : Send {
    type Msg: Encodable + Decodable + Debug + Clone;

    /// Initialize process state if necessary
    fn init(&mut self, _executor_pid: Pid) -> Vec<Envelope<Self::Msg>> {
        Vec::new()
    }

    /// Handle messages from other actors
    fn handle(&mut self,
              msg: Msg<Self::Msg>,
              from: Pid,
              correlation_id: Option<CorrelationId>,
              output: &mut Vec<Envelope<Self::Msg>>);
}
