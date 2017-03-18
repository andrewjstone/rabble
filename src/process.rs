use pid::Pid;
use msg::Msg;
use correlation_id::CorrelationId;
use envelope::Envelope;
use user_msg::UserMsg;

pub trait Process<T: UserMsg> : Send {

    /// Initialize process state if necessary
    fn init(&mut self, _executor_pid: Pid) -> Vec<Envelope<T>> {
        Vec::new()
    }

    /// Handle messages from other actors
    fn handle(&mut self, msg: Msg<T>, from: Pid, cid: CorrelationId) -> &mut Vec<Envelope<T>>;
}
