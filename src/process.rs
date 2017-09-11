use pid::Pid;
use envelope::{Envelope, Msg};
use correlation_id::CorrelationId;

pub trait Process : Send {
    /// Initialize process state if necessary
    fn init(&mut self, _executor_pid: Pid) -> Vec<Envelope> {
        Vec::new()
    }

    /// Handle messages from other actors
    fn handle(&mut self,
              msg: Msg,
              from: Pid,
              correlation_id: Option<CorrelationId>,
              output: &mut Vec<Envelope>);
}
