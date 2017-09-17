use std::mem;
use std::time::Duration;
use std::collections::VecDeque;
use std::sync::mpsc;
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
    pub fn new(pid: Pid, process: Box<Process<T>>) -> Pcb<T> {
        Pcb {
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
    Process { pcb: Option<Pcb<T>>, mailbox: Vec<Envelope<T>> },
    Service { tx: Sender<Envelope<T>> }
}

impl<T> Entry<T> {
    pub fn new_process(pid: Pid, process: Box<Process<T>>) -> Entry<T> {
        Entry::Process {
            pcb: Some(Pcb::new(pid, process)),
            mailbox: Vec::with_capacity(INITIAL_MAILBOX_CAPACITY)
        }
    }

    pub fn new_service(tx: Sender<Envelope<T>>) -> Entry<T> {
        Entry::Service { tx: tx }
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
    map: CHashMap<Pid, Entry<T>>,
    deque: Option<Mutex<VecDeque<Pcb<T>>>>,
}

impl<T> Processes<T> {
    pub fn new(deque: Mutex<VecDeque<Pcb<T>>>) -> Processes<T> {
        Processes {
            map: CHashMap::with_capacity(1024),
            deque: Some(deque)
        }
    }

    /// Create a process control structure for the process and store it in the processes map
    ///
    /// This should only be called when a new process is created.
    /// Returns `Ok(())` if the entry doesn't already exist, `Err(())` otherwise.
    pub fn spawn(&self, pid: Pid, process: Box<Process<T>>) -> Result<(), ()> {
        if self.map.contains_key(&pid) {
            return Err(());
        }
        self.map.insert(pid.clone(), Entry::new_process(pid, process)).unwrap();
        Ok(())
    }

    /// Register the sender for a service so messages can be sent to it
    ///
    /// This should only be called once for a service.
    /// Returns `Ok(())` if the entry doesn't already exist, `Err(())` otherwise.
    pub fn register_service(&self, pid: Pid, tx: Sender<Envelope<T>>) -> Result<(), ()> {
        if self.map.contains_key(&pid) {
            return Err(());
        }
        self.map.insert(pid, Entry::new_service(tx)).unwrap();
        Ok(())
    }

    /// Send an envelope to a process or service at the given pid
    ///
    /// For processes:
    ///     If the pcb is present in the entry, add the envelope to the pcb, remove it from the
    ///     entry and put it on the shared deque so it can be scheduled.
    ///
    ///     If the pcb is not present, add the envelope to the entry mailbox.
    ///
    ///     Returns `Ok(())` if the entry exists, `Err(Envelope)` otherwise
    ///
    /// For services:
    ///     Lookup the sender for the service's channel and send directly
    ///
    ///     Returns `Ok(())` if the entry exists, `Err(Envelope)` otherwise
    ///     Also returns `Err(Envelope)` if the send fails
    ///
    pub fn send(&mut self, envelope: Envelope<T>) -> Result<(), Envelope<T>> {
        // Satisfy the borrow checker by removing the deque from self temporarily
        let deque = self.deque.take().unwrap();

        let result = match self.map.get_mut(&envelope.to) {
            Some(mut entry) => {
                match *entry {
                    Entry::Process { ref mut pcb, ref mut mailbox } => {
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
                    Entry::Service { ref tx } => {
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
    /// If the map entry mailbox contains envelopes then swap it with the Pcb mailbox and return
    /// the Pcb to the caller, as it should remain on the runqueue of the scheduler.
    ///
    /// If the entry mailbox is empty, put the Pcb into the entry and return `None`.
    ///
    /// If the entry no longer exists, it means the process was killed. In that case drop the Pcb
    /// and return `None`.
    pub fn deschedule(&mut self, mut scheduled_pcb: Pcb<T>) -> Option<Pcb<T>> {
        self.map.get_mut(&scheduled_pcb.pid).and_then(|mut entry| {
            if let Entry::Process {ref mut pcb, ref mut mailbox} = *entry {
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
