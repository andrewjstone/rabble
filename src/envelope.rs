use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use system_msg::SystemMsg;
use correlation_id::CorrelationId;

/// Envelopes routable to threads running on the same node as a process.
///
/// These messsages can only be sent on the same node and are not required to implement
/// RustcEncodable and RustcDecodable.
///
/// Some examples of system threads are admin and client servers that require replies from
/// processes, that can then be returned to the user. File management threads are also system
/// processes.
#[derive(Debug, Clone)]
pub struct SystemEnvelope<T> {
    pub to: Pid,
    pub from: Pid,
    pub msg: SystemMsg<T>,
    pub correlation_id: Option<CorrelationId>
}

impl<T> SystemEnvelope<T> {
    pub fn contains_shutdown_msg(&self) -> bool {
        let SystemEnvelope {ref msg, ..} = *self;
        if let SystemMsg::Shutdown = *msg {
            return true;
        }
        false
    }
}

/// Envelopes sent to lightweight processes.
///
/// These are routable between nodes and therefore required to implement RustcEncodable and
/// RustcDecodable.
///
/// Some examples of lightweight processes are members of a consensus protocol or participants in
/// some other distributed protocol that is I/O and not CPU bound.
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct ProcessEnvelope<T: Encodable + Decodable> {
    pub to: Pid,
    pub from: Pid,
    pub msg: T,
    pub correlation_id: Option<CorrelationId>
}

pub enum Envelope<T: Encodable + Decodable, U> {
    Process(ProcessEnvelope<T>),
    System(SystemEnvelope<U>)
}

impl<T: Encodable + Decodable, U> Envelope<T, U> {
    pub fn to(&self) -> &Pid {
        match *self {
            Envelope::Process(ProcessEnvelope {ref to, ..}) => to,
            Envelope::System(SystemEnvelope {ref to, ..}) => to
        }
    }

    pub fn new_system(to: Pid,
                      from: Pid,
                      msg: SystemMsg<U>,
                      c_id: Option<CorrelationId>) -> Envelope<T, U>
    {
        Envelope::System(SystemEnvelope {
            to: to,
            from: from,
            msg: msg,
            correlation_id: c_id
        })
    }

    pub fn new_process(to: Pid, from: Pid, msg: T, c_id: Option<CorrelationId>) -> Envelope<T, U> {
        Envelope::Process(ProcessEnvelope {
            to: to,
            from: from,
            msg: msg,
            correlation_id: c_id
        })
    }
}
