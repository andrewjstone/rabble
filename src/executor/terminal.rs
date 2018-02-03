use std::time::Duration;
use std::sync::mpsc::Sender;
use std::fmt::Debug;
use pid::Pid;
use msg::Msg;
use envelope::Envelope;
use terminal::{Terminal, TimerId};
use cluster::ClusterMsg;
use super::ExecutorMsg;
use serde::{Serialize, Deserialize};

/// The implementation of a terminal for the Executor
///
/// All processes in a rabble process use this type when spawned.
pub struct ExecutorTerminal<T> {
    /// The pid of the process which is currently executing
    /// This process has a mutable ref to this terminal
    current_pid: Pid,
    timer_count: usize,
    tx: Sender<ExecutorMsg<T>>,

    /// All timers are maintained in the cluster server.
    cluster_tx: Sender<ClusterMsg<T>>,
}

impl<T> ExecutorTerminal<T> {
    /// Create a new ExecutorTerminal
    pub fn new(pid: Pid,
           tx: Sender<ExecutorMsg<T>>,
           cluster_tx: Sender<ClusterMsg<T>>) -> ExecutorTerminal<T>
    {
        ExecutorTerminal {
            current_pid: pid,
            timer_count: 0,
            tx: tx,
            cluster_tx: cluster_tx
        }
    }

    /// Set the pid of the currently executing process
   pub fn set_pid(&mut self, pid: Pid) {
        self.current_pid = pid;
    }

    /// Get the next TimerId
    fn timer_id(&mut self) -> TimerId {
        self.timer_count += 1;
        TimerId::new(self.timer_count)
    }
}

impl<'de, T> Terminal<T> for ExecutorTerminal<T>
    where T: Clone + Debug + Serialize + Deserialize<'de>
{
    /// Send an envelope addressed to `to`, with message `Msg::User(msg)`
    ///
    /// The `from` field of the envelope is filled in with self.current_pid
    fn send(&self, to: Pid, msg: T) where T: Clone{
        let envelope = Envelope::new(to, self.current_pid.clone(), Msg::User(msg));

        // This can never fail since the caller holds a ref to the other side of the channel
        self.tx.send(ExecutorMsg::Envelope(envelope)).unwrap();
    }

    /// Start a timer
    ///
    /// A Msg::Timeout(TimerId) will get routed to the process when it expires after `timeout`.
    fn start_timer(&mut self, timeout: Duration) -> TimerId {
        let id = self.timer_id();
        let msg = ClusterMsg::StartTimer(self.current_pid.clone(), id, timeout);

        // Ignore any errors as it means we are shutting down.
        let _ = self.cluster_tx.send(msg);
        id
    }

    /// Cancel a timer
    ///
    /// A user will never receive a timeout for a cancelled TimerId
    fn cancel_timer(&mut self, id: TimerId) {
        let msg = ClusterMsg::CancelTimer(self.current_pid.clone(), id);

        // Ignore any errors as it means we are shutting down.
        let _ = self.cluster_tx.send(msg);
    }
}
