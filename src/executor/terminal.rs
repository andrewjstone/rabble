use std::time::Duration;
use std::fmt::Debug;
use std::vec::Drain;
use pid::Pid;
use msg::Msg;
use envelope::Envelope;
use terminal::{Terminal, TimerId};
use serde::{Serialize, Deserialize};
use ferris::{Wheel, CopyWheel, Resolution};

/// The implementation of a terminal for the Executor
///
/// All processes in a rabble process use this type when spawned.
pub struct ExecutorTerminal<T> {
    /// The pid of the process which is currently executing
    /// This process has a mutable ref to this terminal
    current_pid: Pid,
    timer_count: usize,
    output: Vec<Envelope<T>>,
    timers: CopyWheel<(Pid, TimerId)>,
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone + 'static> ExecutorTerminal<T> {
    /// Create a new ExecutorTerminal
    pub fn new(pid: Pid) -> ExecutorTerminal<T>
    {
        ExecutorTerminal {
            current_pid: pid,
            timer_count: 0,
            output: Vec::new(),
            timers: CopyWheel::new(vec![Resolution::HundredMs,
                                        Resolution::Sec,
                                        Resolution::Min,
                                        Resolution::Hour]),
        }
    }

    /// Set the pid of the currently executing process
    pub fn set_pid(&mut self, pid: Pid) {
        self.current_pid = pid;
    }

    /// Check for expired timers and send timeout messages to processes
    pub fn process_timeouts(&mut self) {
        for (pid, id) in self.timers.expire().into_iter() {
            self.output.push(Envelope::new(pid.clone(), pid, Msg::Timeout(id)));
        }
    }

    /// Return all envelopes that need to be delivered to processes
    pub fn pending(&mut self) -> Drain<Envelope<T>> {
        self.output.drain(..)
    }

    /// Get the next TimerId
    fn timer_id(&mut self) -> TimerId {
        self.timer_count += 1;
        TimerId::new(self.timer_count)
    }
}

impl<'de, T> Terminal<T> for ExecutorTerminal<T>
    where T: Clone + Debug + Serialize + Deserialize<'de> + 'static
{
    /// Send an envelope addressed to `to`, with message `Msg::User(msg)`
    ///
    /// The `from` field of the envelope is filled in with self.current_pid
    fn send(&mut self, to: Pid, msg: T) where T: Clone{
        self.output.push(Envelope::new(to, self.current_pid.clone(), Msg::User(msg)));
    }

    /// Start a timer
    ///
    /// A Msg::Timeout(TimerId) will get routed to the process when it expires after `timeout`.
    fn start_timer(&mut self, timeout: Duration) -> TimerId {
        let id = self.timer_id();
        self.timers.start((self.current_pid.clone(), id), timeout);
        id
    }

    /// Cancel a timer
    ///
    /// A user will never receive a timeout for a cancelled TimerId
    fn cancel_timer(&mut self, id: TimerId) {
        self.timers.stop((self.current_pid.clone(), id));
    }
}
