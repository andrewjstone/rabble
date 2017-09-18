use std::mem;
use std::time::Duration;
use std::collections::VecDeque;
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
use parking_lot::Mutex;
use chashmap::CHashMap;
use amy::{Sender, ChannelError};
use pid::Pid;
use process::Process;
use envelope::Envelope;
use histogram::Histogram;

const INITIAL_MAILBOX_CAPACITY: usize = 16;

/// A Process Control Block
///
/// This struct contains a process, as well as a local mailbox and statistics about process
/// scheduling.
pub struct Pcb<T> {
    /// This is to prevent race conditions where an old Pcb is attempted to be descheduled onto
    /// newer process, such as could happen if the old process was killed and a new one was added,
    /// but the scheduler hadn't yet attempted to deschedule the old one.
    id: usize,
    pid: Pid,
    process: Box<Process<T>>,
    mailbox: Vec<Envelope<T>>,
    total_msgs: u64,
    total_runtime: Duration,
    /// The runtime of the process at a scheduler before being descheduled
    runtime_hist: Histogram,
    /// The number of msgs executed at a time by the scheduler, before descheduling
    msg_hist: Histogram
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
    Service { id: usize, tx: Sender<Envelope<T>> }
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
        Entry::Service { id: id, tx: tx }
    }
}

/// A global concurrent hashmap that stores processes and mailboxes and allows sending
/// messages to local processes.
///
/// This hashmap is primarily used to send messages. When a process is present in the map and a
/// message gets put in its mailbox it is put on a shared work-stealing deque to be picked up by a
/// scheduler. New messages destined for a process that is either in flight or claimed by a
/// scheduler are placed in a mailbox stored in the map. When the scheduled process exhausts its
/// local messages it attempts to swap its empty mailbox with the one in the map. If the map
/// mailbox is empty the process is descheduled. The scheduler may choose to keep the process local
/// temporarily to improve cache locality, but after a period of time it will be put back on the map
/// if it hasn't received any messages.
///
pub struct Processes<T> {
    counter: AtomicUsize,
    map: CHashMap<Pid, Entry<T>>,
    deque: Option<Arc<Mutex<VecDeque<Pcb<T>>>>>,
}

impl<T> Processes<T> {
    pub fn new(deque: Arc<Mutex<VecDeque<Pcb<T>>>>) -> Processes<T> {
        Processes {
            counter: AtomicUsize::new(0),
            map: CHashMap::with_capacity(1024),
            deque: Some(deque)
        }
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
                        return Ok(());
                    }
                    Err(())
                }
                Entry::Service {..} => Err(())
            }
        })
    }

    /// Remove a process from the map
    pub fn kill(&self, pid: &Pid) {
        self.map.remove(pid);
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
                Entry::Process {..} => Err(()),
                Entry::Service { id, ..} => {
                    if id == new_id {
                        return Ok(());
                    }
                    Err(())
                }
            }
        })
    }

    /// Remove the service's sender from the map
    pub fn deregister_service(&self, pid: &Pid) {
        self.map.remove(pid);
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
                    Entry::Process { ref mut pcb, ref mut mailbox, ..} => {
                        if let Some(mut pcb) = pcb.take() {
                            // The process is not scheduled, so push envelope onto the pcb mailbox
                            // and add it to the shared deque so a scheduler can steal it.
                            pcb.mailbox.push(envelope);
                            let mut deque = deque.lock();
                            (*deque).push_back(pcb)
                        } else {
                            // The process is already scheduled, push envelope onto the entry mailbox
                            mailbox.push(envelope);
                        }
                        Ok(())
                    }
                    Entry::Service { ref tx, .. } => {
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

        // Restore the temporarily removed deque
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

mod tests {

    use std::collections::VecDeque;
    use std::sync::Arc;
    use parking_lot::Mutex;
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

    #[test]
    fn process_lifecycle() {
        let deque = Arc::new(Mutex::new(VecDeque::new()));
        let mut processes = Processes::new(deque.clone());
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
            let mut deque = deque.lock();
            let pcb = (*deque).pop_back().unwrap();
            assert_eq!(pcb.mailbox.len(), 1);
            pcb
        };

        // Ensure sending a msg to a process that was already dequed doesn't increase the mailbox of
        // the pcb and the deque is still empty.
        {
            processes.send(envelope()).unwrap();
            let mut deque = deque.lock();
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
