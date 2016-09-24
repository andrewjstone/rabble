use std::io::{self, Read, Write};
use std::fmt::Debug;
use std::os::unix::io::AsRawFd;
use rustc_serialize::{Encodable, Decodable};
use node::Node;
use envelope::{Envelope, SystemEnvelope};
use errors::*;
use correlation_id::CorrelationId;
use pid::Pid;
use protocol::Protocol;

/// Implement this for a specific connection handler
pub trait Connection : Sized {
    type ProcessMsg: Encodable + Decodable;
    type SystemUserMsg: Debug + Clone;
    type ClientMsg: Encodable + Decodable + Debug;
    type Protocol: Protocol;

    fn new(pid: Pid, id: usize) -> Self;

    fn handle_system_envelope(&mut self,
                              SystemEnvelope<Self::SystemUserMsg>) -> Vec<ConnectionMsg<Self>>;

    fn handle_network_msg(&mut self, Self::ClientMsg) -> Vec<ConnectionMsg<Self>>;

    // TODO: Implement this for all users if possible
    /// Read and decode a single message at a time.
    ///
    /// This function should be called until it returns Ok(None) in which case there is no more data
    /// left to return. For async sockets this signals that the socket should be re-registered.
    fn read_msg<T: Read>(&mut self, reader: &mut T) -> Result<Option<Self::ClientMsg>>;

    // TODO: Implement this for all users if possible
    /// Write out as much pending data as possible. Append `msg` to the pending data if not `None`.
    ///
    /// Return whether the writer is still writable.
    fn write_msgs<T: Write>(&mut self,
                            writer: &mut T,
                            msg: Option<&Self::ClientMsg>) -> Result<bool>;

}

/// Connection messages are returned from the callback functions for a Connection.
///
/// These messages can be either an envelope as gets used in the rest of the system or a message
/// specific to this service that can be serialized and sent to a client on the other end of the
/// connection.
pub enum ConnectionMsg<C: Connection>
{
    Envelope(Envelope<C::ProcessMsg, C::SystemUserMsg>),
    ClientMsg(C::ClientMsg, CorrelationId)
}
