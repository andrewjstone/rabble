use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::collections::VecDeque;
use std::time::Duration;
use parking_lot::{Mutex, Condvar};
use coco::deque;
use slog::Logger;
use pid::Pid;
use envelope::Envelope;
use processes::{Processes, Pcb};
use cluster::ClusterMsg;

const ALLOW_STEAL_MIN_MESSAGES: u64 = 10*1024;

/// A scheduler runs on a single thread and schedules processes
///
/// Schedulers start in an empty state with no processes allocated to them. When a message is sent
/// to a process, via the `Processes` object, the process control block (`Pcb`) containing the
/// process is placed on a deque.An available scheduler will pull from the deque and put the Pcb on
/// // // we can learn the new config. We can then announce the reconfiguration to the namespace manager, and restart the recovery process with that config.
/// it//s run queue. When it is time to execute the process (i.e. handle messages in its mailbox) it
/// will handle a subset of messages due to scheduler policy, then put the Pcb on the back of the
/// run queue.
///
/// When the scheduler has handled all messages in the Pcb mailbox, it attempts to deschedule the
/// process by putting the Pcb back into the Processes map. If their are pending messages in the map
/// entry mailbox, they are transferred to the Pcb mailbox and the process is put back on the run
/// queue.
///
/// As schedulers can have a mismatched amount of processes, and therefore work to do, there needs
/// to be a way to balance this work in order to keep all schedulers busy. When a scheduler has no
/// work to do, it will attempt to steal a process from another scheduler's run queue. This allows
/// execution of the stolen process to proceed before it would run on the original scheduler. It
/// also helps balance work among threads.  However, it reduces process locality, and therefore a
/// tradeoff exists. This initial implementation always allows work stealing, but in the future it
/// may be optimized to limit it to only when necessary so as to preserve locality.
///
///
/// Work stealing works well when all processes are allocated to a single scheduler or there is a
/// lot of work to do and proceses get spread out across schedulers. However, we can end up in the
/// opposite situation where each scheduler has a bunch of processes running with minimal messages
/// in each process and processes are potentially communicating with other processes on different
/// schedulers. The schedulers never run out of work, but they do more work then necessary due to
/// synchronization overhead and lack of locality. In this case, the best course of action is to
/// migrate processes to fewer threads. This initial implementation does not however implement
/// process migration. It will be added in the future when a design is worked out. For now, lightly
/// loaded systems may chose to reduce the number of schedulers being run.
pub struct Scheduler<T> {
    /// Schedulers have pids and can be sent messages
    pid: Pid,

    /// The global process map shared among all schedulers and other senders
    processes: Processes<T>,

    /// Channel to the Cluster Server used for sending envelopes to other nodes
    cluster_tx: Sender<ClusterMsg<T>>,

    /// Processes get put on the queue in the first element of this pair when they first receive a
    /// message.  A scheduler will select a process, put it on the run_queue, and continue
    /// processing it until it runs out of messages or another scheduler steals it. The Condvar is
    /// used to wake up sleeping scheduler threads that have no work to do.
    unscheduled: Arc<(Mutex<VecDeque<Pcb<T>>>, Condvar)>,

    /// The number of messages handled by this scheduler for its entire lifetime
    total_msgs: u64,

    /// The number of messages in the mailboxes of all Pcbs on this scheduler
    msgs_queued: Arc<AtomicUsize>,

    /// Currently active processes
    run_queue: deque::Worker<Pcb<T>>,

    /// This schedulers' stealing half of the run_queue deque
    stealer: deque::Stealer<Pcb<T>>,

    /// Other schedulers' (pid, run_queue) pairs
    peers: Vec<(Pid, deque::Stealer<Pcb<T>>)>,

    /// The index of the last stolen peer. We steal in a round robin fashion.
    last_stolen: usize,

    /// The system logger
    logger: Logger
}

impl<T> Scheduler<T> {
    /// Create a new scheduler
    pub fn new(pid: Pid,
               processes: Processes<T>,
               cluster_tx: Sender<ClusterMsg<T>>,
               logger: Logger) -> Scheduler<T>
    {
        let name = pid.name.clone();
        let (worker, stealer) = deque::new();
        let unscheduled = processes.clone_deque();
        Scheduler {
            pid: pid,
            processes: processes,
            cluster_tx: cluster_tx,
            unscheduled: unscheduled,
            total_msgs: 0,
            msgs_queued: Arc::new(AtomicUsize::new(0)),
            run_queue: worker,
            stealer: stealer,
            peers: Vec::new(),
            last_stolen: 0,
            logger: logger.new(o!("component" => name))
        }
    }

    /// Retrieve the (pid, stealer) pair of this scheduler
    pub fn stealer(&self) -> (Pid, deque::Stealer<Pcb<T>>) {
        (self.pid.clone(), self.stealer.clone())
    }

    /// We only have a fixed number of schedulers at startup
    /// Pass in their stealers before calling `run`.
    pub fn set_peers(&mut self, peers: Vec<(Pid, deque::Stealer<Pcb<T>>)>) {
        self.peers = peers;
    }

    /// Run the scheduler
    ///
    /// This function blocks indefinitely
    pub fn run(mut self) {
        // TODO: This will no longer be constant when we have a timer wheel
        let sleep_time = Duration::from_millis(100);

        info!(self.logger, "Starting scheduler");

        // Output envelopes from calling a process's handle method get placed here temporarily.
        let mut output = Vec::with_capacity(16);

        loop {

            // If the system is shutting down, simply exit the loop
            if self.processes.is_shutdown() {
                break;
            }

            // 1. Check the global deque for any new Pcbs
            if let Some(pcb) = self.take_unscheduled() {
                trace!(self.logger, "process scheduled";
                       "id" => pcb.id,
                       "pid" => pcb.pid.to_string(),
                       "mailbox_len" => pcb.mailbox.len());
                self.run_queue.push(pcb)
            }
            trace!(self.logger, "Before run queue steal");

            // 2. Attempt to execute a process on the run_queue
            // TODO: We drain all messages in the Pcb mailbox at once, but we may want to limit them
            // instead for fairness. The limit should be added to the scheduler policy.
            match self.run_queue.steal() {
                Some(mut pcb) => {
                    trace!(self.logger, "Scheduling a process");

                    // Handle all messages in the pcb mailbox
                    for Envelope {from, msg, correlation_id, ..} in pcb.mailbox.drain(..) {
                        trace!(self.logger, "handling a message");
                        pcb.process.handle(msg, from, correlation_id, &mut output);

                        // Send any outgoing messages from the currently executing process
                        for envelope in output.drain(..) {
                            if envelope.to.node == self.pid.node {
                                // Drop messages without a receiver
                                let _ = self.processes.send(envelope);
                            } else {
                                if let Err(_) =
                                    self.cluster_tx.send(ClusterMsg::Envelope(envelope)) {
                                        // The node is shutting down.
                                        info!(self.logger, "shutting down");
                                        return;
                                }
                            }
                        }
                    }

                    // Attempt to deschedule the process. This will return Some(pcb) if there were
                    // messages waiting in the processes entry mailbox. Those messages will be swapped
                    // into the pcb so that it can be put back on the run_queue.
                    let id = pcb.id;
                    let pidstr = pcb.pid.to_string();
                    let mailbox_len = pcb.mailbox.len();
                    if let Some(pcb) = self.processes.deschedule(pcb) {
                        trace!(self.logger, "mailbox swapped";
                               "id" => id,
                               "pid" => pidstr,
                               "mailbox_len" => mailbox_len);
                        self.run_queue.push(pcb);
                    } else {
                        trace!(self.logger, "process descheduled";
                               "id" => id,
                               "pid" => pidstr,
                               "mailbox_len" => mailbox_len);
                    }
                }
                None => {
                    trace!(self.logger, "No processes to schedule");
                    if !self.peers.is_empty() {
                        // There are other schedulers in this system
                        // We have no more work to do. Attempt to steal a pcb
                        let start = self.last_stolen + 1;
                        let mut current = start;
                        // TODO: Theoretically we could end up with 2 schedulers continuously stealing the
                        // same pcb and never acxtually handling messages (livelock). We should check
                        // that there are at least a few processes on each scheduler before attempting
                        // to steal from it. We can use atomics for this. These counters will likely
                        // also be used as part of a migration strategy in the future.
                        loop {
                            match self.peers[current].1.steal() {
                                None => {
                                    current = (current + 1) % self.peers.len();
                                    if current == start {
                                        trace!(self.logger, "No processes to steal");

                                        // We have wrapped around
                                        break;
                                    }
                                }
                                Some(pcb) => {
                                    trace!(self.logger, "process stolen";
                                           "id" => pcb.id,
                                           "pid" => pcb.pid.to_string(),
                                           "mailbox_len" => pcb.mailbox.len());
                                    self.run_queue.push(pcb);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // If there is no more work to do then go to sleep
            if self.run_queue.len() == 0 {
                // TODO:  We may want to consider waiting on a scheduler to signal when it's
                // outstanding work has crossed some threshold. Therefore, awake schedulers would
                // take off the unscheduled queue and only wake up sleeping schedulers when they
                // become overloaded.
                info!(self.logger, "Scheduler sleeping");
                let mut deque = self.unscheduled.0.lock();
                self.unscheduled.1.wait_for(&mut deque, sleep_time);
            }
        }
    }

    fn take_unscheduled(&mut self) -> Option<Pcb<T>> {
        trace!(self.logger, "Take unscheduled");
        let mut unscheduled = self.unscheduled.0.lock();
        (*unscheduled).pop_front()
    }
}
