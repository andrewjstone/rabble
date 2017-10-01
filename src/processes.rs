use std::mem;
use std::time::Duration;
use std::collections::VecDeque;
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use parking_lot::{Mutex, Condvar};
use chashmap::CHashMap;
use amy::{Sender, ChannelError};
use slog::Logger;
use pid::Pid;
use process::Process;
use envelope::Envelope;
use histogram::Histogram;

const INITIAL_MAILBOX_CAPACITY: usize = 16;
const INITIAL_DEQUE_CAPACITY: usize = 128;

/// A Process Control Block
///
/// This struct contains a process, as well as a local mailbox and statistics about process
/// scheduling.
pub struct Pcb<T> {
    /// This is to prevent race conditions where an old Pcb is attempted to be descheduled onto
    /// newer process, such as could happen if the old process was killed and a new one was added,
    /// but the scheduler hadn't yet attempted to deschedule the old one.
    pub id: usize,
    pub pid: Pid,
    pub process: Box<Process<T>>,
    pub mailbox: Vec<Envelope<T>>,
    pub total_msgs: u64,
    pub total_runtime: Duration,
    /// The runtime of the process at a scheduler before being descheduled
    pub runtime_hist: Histogram,
    /// The number of msgs executed at a time by the scheduler, before descheduling
    pub msg_hist: Histogram
}

impl<T> Pcb<T> {
    pub fn new(id: usize, pid: Pid, process: Box<Process<T>>) -> Pcb<T> {
        Pcb {
            id: id,
            pid: pid,
            process: process,
            mailbox: Vec::with_capacity(INITIAL_MAILBOX_CAPACITY),
            total_msgs: 0,
            total_runtime: Duration::new(0, 0),
            runtime_hist: Histogram::new(),
            msg_hist: Histogram::new()
        }
    }
}

/// An entry in the Processes map
pub enum Entry<T> {
    Process { id: usize, pcb: Option<Pcb<T>>, mailbox: Vec<Envelope<T>> },
    // Senders are Send, but not Sync. Unfortunately this requires wrapping a non-blocking channel
    // in a muted :(
    // TODO: Either send all service messages to a single routing thread over a channel that is not
    // stored in an entry so that we don't require a mutex,  or use a type of sender that is sync in
    // the first place.
    Service { id: usize, tx: Arc<Mutex<Sender<Envelope<T>>>> }
}

impl<T> Entry<T> {
    pub fn new_process(id: usize, pid: Pid, process: Box<Process<T>>) -> Entry<T> {
        Entry::Process {
            id: id,
            pcb: Some(Pcb::new(id, pid, process)),
            mailbox: Vec::with_capacity(INITIAL_MAILBOX_CAPACITY)
        }
    }

    pub fn new_service(id: usize, tx: Sender<Envelope<T>>) -> Entry<T> {
        Entry::Service { id: id, tx: Arc::new(Mutex::new(tx)) }
    }
}

/// A global concurrent hashmap that stores processes and mailboxes and allows sending
/// messages to local processes.
///
/// This hashmap is primarily used to send messages. When a process ispresent in the map and a
/// message gets put in its mailbox it is put on a shared work-stealing deque to be picked up by a
/// scheduler. New messages destined for a process that is either in flight or claimed by a
/// scheduler are placed in a mailbox stored in the map. When the scheduled process exhausts its
/// local messages it attempts to swap its empty mailbox with the one in the map. If the map
/// mailbox is empty the process is descheduled. The scheduler may choose to keep the process local
/// temporarily to improve cache locality, but after a period of time it will be put back on the map
/// if it hasn't received any messages.
#[derive(Clone)]
pub struct Processes<T> {
    counter: Arc<AtomicUsize>,
    map: Arc<CHashMap<Pid, Entry<T>>>,
    deque: Option<Arc<(Mutex<VecDeque<Pcb<T>>>, Condvar)>>,
    logger: Logger,
    shutdown: Arc<AtomicBool>
}

impl<T> Processes<T> {
    pub fn new(logger: Logger) -> Processes<T> {
        Processes {
            counter: Arc::new(AtomicUsize::new(0)),
            map: Arc::new(CHashMap::with_capacity(1024)),
            deque: Some(Arc::new((Mutex::new(VecDeque::with_capacity(INITIAL_DEQUE_CAPACITY)),
                                  Condvar::new()))),
            logger: logger.new(o!("component" => "Processes")),
            shutdown: Arc::new(AtomicBool::new(false))
        }
    }

    /// Return a cloned deque and its associated condvar that gets notified when a process is
    /// scheduled.
    pub fn clone_deque(&self) -> Arc<(Mutex<VecDeque<Pcb<T>>>, Condvar)> {
        self.deque.clone().unwrap()
    }

    /// Let all schedulers know that the system is shutting down
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Return true if the system is shutting down
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    /// Create a process control structure for the process and store it in the processes map
    ///
    /// This should only be called when a new process is created.
    /// Returns `Ok(())` if the entry doesn't already exist, `Err(())` otherwise.
    pub fn spawn(&self, pid: Pid, process: Box<Process<T>>) -> Result<(), ()> {
        // We need to ensure the key doesn't exist before inserting it.
        // We don't use `insert`, because we don't want to replace any existing keys. We can't just
        // call `self.map.contains_key(&pid)` because after that call the lock is not held and
        // another key can insert into the map.
        // Therefore we upsert and then check to see the same value exists as we tried to insert.
        // That way we can return an error if we tried to insert but a key already existed.
        let new_id = self.counter.fetch_add(1, Ordering::SeqCst);
        self.map.upsert(pid.clone(), || Entry::new_process(new_id, pid.clone(), process), |_| {});
        self.map.get(&pid).map_or(Err(()), |entry| {
            match *entry {
                Entry::Process { id, ..} => {
                    if id == new_id {
                        info!(self.logger, "Spawn process succeded"; "id" => id, "pid" => pid.to_string());
                        return Ok(());
                    }
                    error!(self.logger, "Spawn process failed"; "id" => id, "pid" => pid.to_string());
                    Err(())
                }
                Entry::Service {id, ..} => {
                    error!(self.logger, "Spawn process failed"; "id" => id, "pid" => pid.to_string());
                    Err(())
                }
            }
        })
    }

    /// Remove a process from the map
    pub fn kill(&self, pid: &Pid) {
        self.map.remove(pid).map(|entry| {
            match entry {
                Entry::Process {id, ..} =>
                    info!(self.logger, "Kill process"; "id" => id, "pid" => pid.to_string()),
                Entry::Service {id, ..} =>
                    info!(self.logger, "Deregister service from processes map (kill)";
                          "id" => id, "pid" => pid.to_string())
            }
        });
    }

    /// Register the sender for a service so messages can be sent to it
    ///
    /// This should only be called once for a service.
    /// Returns `Ok(())` if the entry doesn't already exist, `Err(())` otherwise.
    pub fn register_service(&self, pid: Pid, tx: Sender<Envelope<T>>) -> Result<(), ()> {
        // We need to ensure the key doesn't exist before inserting it.
        // We don't use `insert`, because we don't want to replace any existing keys. We can't just
        // call `self.map.contains_key(&pid)` because after that call the lock is not held and
        // another key can insert into the map.
        // Therefore we upsert and then check to see the same value exists as we tried to insert.
        // That way we can return an error if we tried to insert but a key already existed.
        let new_id = self.counter.fetch_add(1, Ordering::SeqCst);
        self.map.upsert(pid.clone(), || Entry::new_service(new_id, tx), |_| {});
        self.map.get(&pid).map_or(Err(()), |entry| {
            match *entry {
                Entry::Process {id, ..} => {
                    info!(self.logger, "Register service failed"; "id" => id, "pid" => pid.to_string());
                    Err(())
                }
                Entry::Service { id, ..} => {
                    if id == new_id {
                        info!(self.logger, "Register service succeded"; "id" => id, "pid" => pid.to_string());
                        return Ok(());
                    }
                    info!(self.logger, "Register service failed"; "id" => id, "pid" => pid.to_string());
                    Err(())
                }
            }
        })
    }

    /// Send an envelope to a process or service at the given pid
    ///
    /// For processes:
    ///   If the pcb is present in the entry, add the envelope to the pcb, remove it from the
    ///   entry and put it on the shared deque so it can be scheduled.
    ///
    ///   If the pcb is not present, add the envelope to the entry mailbox.
    ///
    ///   Returns `Ok(())` if the entry exists, `Err(Envelope)` otherwise
    ///
    /// For services:
    ///   Lookup the sender for the service's channel and send directly
    ///
    ///   Returns `Ok(())` if the entry exists, `Err(Envelope)` otherwise
    ///   Also returns `Err(Envelope)` if the send fails
    ///
    pub fn send(&mut self, envelope: Envelope<T>) -> Result<(), Envelope<T>> {
        // Satisfy the borrow checker by removing the deque from self temporarily
        let deque = self.deque.take().unwrap();

        let result = match self.map.get_mut(&envelope.to) {
            Some(mut entry) => {
                match *entry {
                    Entry::Process { ref mut pcb, ref mut mailbox, id} => {
                        if let Some(mut pcb) = pcb.take() {
                            trace!(self.logger, "Send to descheduled process";
                                   "id" => id,
                                   "from" => envelope.from.to_string(),
                                   "to" => envelope.to.to_string());
                            // The process is not scheduled, so push envelope onto the pcb mailbox
                            // and add it to the shared deque so a scheduler can steal it.
                            pcb.mailbox.push(envelope);
                            let mut _deque = deque.0.lock();
                            (*_deque).push_back(pcb);
                            deque.1.notify_one();
                        } else {
                            // The process is already scheduled, push envelope onto the entry mailbox
                            trace!(self.logger, "Send to already scheduled process";
                                   "id" => id,
                                   "from" => envelope.from.to_string(),
                                   "to" => envelope.to.to_string());
                            mailbox.push(envelope);
                        }
                        Ok(())
                    }
                    Entry::Service { ref tx, id} => {
                        let tx = tx.lock();
                        trace!(self.logger, "Send to service";
                               "id" => id,
                               "from" => envelope.from.to_string(),
                               "to" => envelope.to.to_string());
                        tx.send(envelope).map_err(|e| {
                            if let ChannelError::SendError(mpsc::SendError(envelope)) = e {
                                return envelope;
                            }
                            unreachable!()
                        })
                    }
                }
            }
            None => Err(envelope)
        };

        // Restore the temporarily removed deque/condvar pair
        self.deque = Some(deque);
        result
    }

    /// Attempt to deschedule a process
    ///
    /// If the map entry mailbox contains envelopes then swap it with the `scheduled_pcb` mailbox
    /// and return the `scheduled_pcb` to the caller, as it should remain on the runqueue of the
    /// scheduler. The scheduler will only attempt to deschedule processes with no messages in the
    /// Pcb mailbox.
    ///
    /// If the entry mailbox is empty, put `scheduled_pcb` into the entry and return `None`.
    ///
    /// If the entry no longer exists, or the id of the entry differs from that of `scheduled_pcb`,
    /// it means the process was killed. In that case drop `scheduled_pcb` and return `None`.
    pub fn deschedule(&mut self, mut scheduled_pcb: Pcb<T>) -> Option<Pcb<T>> {
        assert!(scheduled_pcb.mailbox.is_empty());
        self.map.get_mut(&scheduled_pcb.pid).and_then(|mut entry| {
            if let Entry::Process {id, ref mut pcb, ref mut mailbox} = *entry {
                if scheduled_pcb.id != id {
                    // This process was already killed and replaced, so drop scheduled_pcb
                    return None;
                }
                if mailbox.is_empty() {
                    *pcb = Some(scheduled_pcb);
                    return None;
                }
                mem::swap(mailbox, &mut scheduled_pcb.mailbox);
                return Some(scheduled_pcb);
            }
            // This is a weird case where a process with some pid has been replaced with a service
            // of the same pid. Since registration of the service would fail if the process hadn't
            // been removed, we just go ahead and drop the scheduled_pcb.
            None
        })
    }
}

#[cfg(test)]
mod tests {

    use std::collections::VecDeque;
    use std::sync::Arc;
    use parking_lot::Mutex;
    use slog_term;
    use slog_async;
    use slog::{self, Drain};
    use super::*;
    use process::Process;
    use node_id::NodeId;
    use correlation_id::CorrelationId;
    use pid::Pid;
    use msg::Msg;

    struct TestProcess;

    impl Process<()> for TestProcess {
        fn handle(&mut self,
                  _: Msg<()>,
                  _: Pid,
                  _: Option<CorrelationId>,
                  _: &mut Vec<Envelope<()>>) {
        }
    }

    fn node() -> NodeId {
        NodeId {
            name: "test-node".to_owned(),
            addr: "127.0.0.1:5000".to_owned()
        }
    }

    fn pid(name: &str) -> Pid {
        Pid {
            name: name.to_owned(),
            group: None,
            node: node()
        }
    }

    fn envelope() -> Envelope<()> {
        Envelope::new(pid("pid1"), pid("tester"), Msg::User(()), None)
    }

    fn logger() -> Logger {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        Logger::root(drain, o!())
    }

    #[test]
    fn process_lifecycle() {
        let mut processes = Processes::new(logger());
        let mut deque = processes.clone_deque();
        let pid1 = pid("pid1");
        processes.spawn(pid1.clone(), Box::new(TestProcess) as Box<Process<()>>).unwrap();

        // Attempting to spawn a process with the same pid should fail
        assert_eq!(Err(()), processes.spawn(pid1, Box::new(TestProcess) as Box<Process<()>>));

        // Send the first message to the process. This should cause the Pcb to be sent over the
        // deque.
        processes.send(envelope()).unwrap();

        // Ensure the pcb is put on the deque for scheduling and the pcb contains the envelope in
        // its mailbox
        let mut pcb = {
            // The extra scope is to ensure the deque goes out of scope and gets unlocked
            let mut deque = deque.0.lock();
            let pcb = (*deque).pop_back().unwrap();
            assert_eq!(pcb.mailbox.len(), 1);
            pcb
        };

        // Ensure sending a msg to a process that was already dequed doesn't increase the mailbox of
        // the pcb and the deque is still empty.
        {
            processes.send(envelope()).unwrap();
            let mut deque = deque.0.lock();
            assert_eq!((*deque).len(), 0);
            assert_eq!(pcb.mailbox.len(), 1);
        }

        // Remove the message off the mailbox (this is an invariant any scheduler must maintain)
        pcb.mailbox.pop();

        // Try to deschedule the process. This should fail because there the second send inserted an
        // envelope into the entry mailbox. The deschedule should swap the empty mailbox of the pcb
        // with that of the one in the entry and return the pcb.
        let pcb = processes.deschedule(pcb);
        assert!(pcb.is_some());
        let mut pcb = pcb.unwrap();
        assert_eq!(pcb.mailbox.len(), 1);

        // Pop the last message off the pcb and attempt to deschedule again. This time the
        // deschedule should succeed and `None` should be returned. We only sent two messages
        // to the process and we have already popped both of them.
        pcb.mailbox.pop();
        assert!(processes.deschedule(pcb).is_none());

        // Ensure killing a process removes it from the map
        assert_eq!(processes.map.len(), 1);
        processes.kill(&pid("pid1"));
        assert_eq!(processes.map.len(), 0);

        // Sending to a non-existant process should fail
        assert_matches!(processes.send(envelope()), Err(_));
    }
}
