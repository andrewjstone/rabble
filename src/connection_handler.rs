use std::io::{self, Read, Write};
use std::fmt::Debug;
use std::os::unix::io::AsRawFd;
use rustc_serialize::{Encodable, Decodable};
use node::Node;
use envelope::{Envelope, SystemEnvelope};
use errors::*;
use correlation_id::CorrelationId;
use pid::Pid;

/// Implement this for a specific connection handler
pub trait ConnectionHandler : Sized {
    type ProcessMsg: Encodable + Decodable;
    type SystemUserMsg: Debug + Clone;
    type ClientMsg: Encodable + Decodable + Debug;

    fn new(pid: Pid, id: usize) -> Self;

    fn handle_system_envelope(&mut self,
                              SystemEnvelope<Self::SystemUserMsg>) -> &mut Vec<ConnectionMsg<Self>>;

    fn handle_network_msg(&mut self, Self::ClientMsg) -> &mut Vec<ConnectionMsg<Self>>;
}

/// Connection messages are returned from the callback functions for a Connection.
///
/// These messages can be either an envelope as gets used in the rest of the system or a message
/// specific to this service that can be serialized and sent to a client on the other end of the
/// connection.
pub enum ConnectionMsg<C: ConnectionHandler>
{
    Envelope(Envelope<C::ProcessMsg, C::SystemUserMsg>),
    Client(C::ClientMsg, CorrelationId)
}
