use std::mem;
use std::time::Duration;
use std::collections::VecDeque;
use parking_lot::Mutex;
use chashmap::CHashMap;
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
pub struct Entry<T> {
    pcb: Option<Pcb<T>>,
    mailbox: Vec<Envelope<T>>
}

impl<T> Entry<T> {
    pub fn new(pid: Pid, process: Box<Process<T>>) -> Entry<T> {
        Entry {
            pcb: Some(Pcb::new(pid, process)),
            mailbox: Vec::with_capacity(INITIAL_MAILBOX_CAPACITY)
        }
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
    /// Returns `Ok(())` if the process doesn't already exist, `Err(())` otherwise.
    pub fn spawn(&self, pid: Pid, process: Box<Process<T>>) -> Result<(), ()> {
        self.map.insert(pid.clone(), Entry::new(pid, process))
                .map_or(Ok(()), |_| Err(()))
    }

    /// Send an envelope
    ///
    /// If the pcb is present in the entry, add the envelope to the pcb, remove it from the entry
    /// and put it on the shared deque so it can be scheduled.
    ///
    /// If the pcb is not present, add the envelope to the entry mailbox.
    ///
    /// Returns `Ok(())` if the entry exists, `Err(())` otherwise
    pub fn send(&mut self, envelope: Envelope<T>) -> Result<(), ()> {
        // Satisfy the borrow checker by removing the deque from self temporarily
        let deque = self.deque.take().unwrap();

        let result = self.map.get_mut(&envelope.to).map(|mut entry| {
            if let Some(mut pcb) = entry.pcb.take() {
                // the process is not scheduled, so push envelope onto the pcb mailbox
                // and add it to the shared deque so a scheduler can steal it
                pcb.mailbox.push(envelope);
                let mut deque = deque.lock();
                (*deque).push_back(pcb)
            } else {
                // the process is already scheduled, push envelope onto the entry mailbox
                entry.mailbox.push(envelope);
            }
        }).ok_or(());

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
    pub fn deschedule(&mut self, pcb: Pcb<T>) -> Option<Pcb<T>> {
        unimplemented!()
    }
}
