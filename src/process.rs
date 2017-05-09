use pid::Pid;
use msg::Msg;
use envelope::Envelope;
use correlation_id::CorrelationId;

pub trait Process<T> : Send {
    /// Initialize process state if necessary
    fn init(&mut self, _executor_pid: Pid) -> Vec<Envelope<T>> {
        Vec::new()
    }

    /// Handle messages from other actors
    fn handle(&mut self,
              msg: Msg<T>,
              from: Pid,
              correlation_id: Option<CorrelationId>,
              output: &mut Vec<Envelope<T>>);
}
