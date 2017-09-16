//! A global concurrent hashmap that stores all processes' mailboxes by Pid
//!
//! This hashmap is primarily used to by scheduler threads to send messages.  It is not guaranteed
//! that all messages for a given process reside here.  The reason for this is that scheduled
//! processes take all messages out at once in a double buffering strategy. They may be handling
//! messages already sent. Furthermore, in order to maintain cache locality and minimize locking, if
//! a message is sent between processes running on the same scheduler, it is placed directly on the
//! scheduler local mailbox of the process rather than the global mailbox.
//!

use std::mem;
use chashmap::CHashMap;
use pid::Pid;
use envelope::Envelope;

const INITIAL_MAILBOX_LEN: usize = 16;

#[derive(Debug, Clone)]
pub struct Mailboxes<T> {
    map: CHashMap<Pid, Vec<Envelope<T>>>
}

impl<T> Mailboxes<T> {
    pub fn new() -> Mailboxes<T> {
        Mailboxes {
            map: CHashMap::with_capacity(1024)
        }
    }

    /// Create a new mailbox
    ///
    /// This should only be called when a new process is created.
    /// Returns `Ok(())` if the process doesn't already exist, `Err(())` otherwise.
    pub fn create_mailbox(&self, pid: Pid) -> Result<(), ()> {
        self.map.insert(pid, Vec::with_capacity(INITIAL_MAILBOX_LEN))
            .map_or(Ok(()), |_| Err(()))
    }

    /// Send an envelope
    ///
    /// Returns `Ok(())` if the mailbox exists, `Err(())` otherwise
    pub fn send(&self, envelope: Envelope<T>) -> Result<(), ()> {
        self.map.get_mut(&envelope.to).map(|mut mailbox| {
            mailbox.push(envelope)
        }).ok_or(())
    }

    /// Swap out the existing mailbox vec with another, returning the original
    ///
    /// This function should only be called by schedulers when executing a process.
    /// Returns `Err(())` if the mailbox does not exist
    pub fn replace(&self, pid: &Pid, vec: Vec<Envelope<T>>) -> Result<Vec<Envelope<T>>, ()> {
        self.map.get_mut(pid).map(|mut mailbox| {
            mem::replace(&mut *mailbox, vec)
        }).ok_or(())
    }
}
