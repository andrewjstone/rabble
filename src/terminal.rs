use std::time::Duration;
use pid::Pid;

/// An opaque handle to a timer
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TimerId(usize);

impl TimerId {
    pub fn new(value: usize) -> TimerId {
        TimerId(value)
    }
}

/// A terminal is how a process interacts with the actor system.
///
/// It uses it to both send messages and manage timers.
pub trait Terminal<T> {
    fn send(&mut self, to: Pid, msg: T);
    fn start_timer(&mut self, Duration) -> TimerId;
    fn cancel_timer(&mut self, TimerId);
}
