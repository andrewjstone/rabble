use std::io::{self, Read, Write};
use std::fmt::Debug;
use std::os::unix::io::AsRawFd;
use rustc_serialize::{Encodable, Decodable};
use node::Node;
use envelope::{Envelope, SystemEnvelope};
use errors::*;
use correlation_id::CorrelationId;
use pid::Pid;

/// This trait provides for reading framed messages from a `Read` type, decoding them and
/// returning them. It buffers incomplete messages. Reading of only a single message of a time is to
/// allow for strategies that prevent starvation of other readers.
pub trait MsgReader {
    type Msg;

    /// Create a new MsgReader
    fn new() -> Self;

    /// Read and decode a single message at a time.
    ///
    /// This function should be called until it returns Ok(None) in which case there is no more data
    /// left to return. For async sockets this signals that the socket should be re-registered.
    fn read_msg<T: Read>(&mut self, reader: &mut T) -> Result<Option<Self::Msg>>;
}

/// This trait provides for serializing and framing messages, and then writing them to a `Write`
/// type. When a complete message cannot be sent it is buffered for when the `Write` type is next
/// writable.
///
/// We write all possible data to the writer until it blocks or there is no more data to be written.
/// Since all output is in response to input, we don't worry about starvation of writers. In order
/// to minimize memory consumption we just write as much as possible and worry about starvation
/// management on the reader side.
///
pub trait MsgWriter {
    type Msg;

    /// Create a new MsgWriter
    fn new() -> Self;

    /// Write out as much pending data as possible. Append `msg` to the pending data if not `None`.
    fn write_msgs<T: Write>(&mut self, writer: &mut T, msg: Option<&Self::Msg>) -> Result<bool>;
}

/// A simple constructor for a Generic State
pub trait State {
    fn new() -> Self;
}

/// A trait to bundle all generic parameters used in a connection
///
/// This trait also contains the 2 callback functions needed for a connection
pub trait ConnectionTypes {
    type State: State;
    type Socket: Read + Write + AsRawFd;
    type ProcessMsg: Encodable + Decodable;
    type SystemMsgTypeParameter: Debug + Clone;
    type ClientMsg: Encodable + Decodable + Debug;
    type MsgWriter: MsgWriter<Msg=Self::ClientMsg> + 'static;
    type MsgReader: MsgReader<Msg=Self::ClientMsg> + 'static;

    fn system_envelope_callback(&mut Self::State,
                                SystemEnvelope<Self::SystemMsgTypeParameter>)
        -> Vec<ConnectionMsg<Self::ProcessMsg,
                             Self::SystemMsgTypeParameter,
                             Self::ClientMsg>>;
    fn network_msg_callback(&mut Self::State,
                            Self::ClientMsg)
        -> Vec<ConnectionMsg<Self::ProcessMsg,
                             Self::SystemMsgTypeParameter,
                             Self::ClientMsg>>;
}

/// Connection messages are returned from the callback functions for a Connection.
///
/// These messages can be either an envelope as gets used in the rest of the system or a message
/// specific to this service that can be serialized and sent to a client on the other end of the
/// connection.
pub enum ConnectionMsg<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: Encodable + Decodable + Debug
{
    Envelope(Envelope<T, U>),
    ClientMsg(C, CorrelationId)
}

pub type SystemEnvelopeCallback<T: ConnectionTypes> =
    fn(&mut T::State,
       SystemEnvelope<T::SystemMsgTypeParameter>)
    -> Vec<ConnectionMsg<T::ProcessMsg, T::SystemMsgTypeParameter, T::ClientMsg>>;

pub type NetworkMsgCallback<T: ConnectionTypes> =
    fn(&mut T::State, T::ClientMsg)
    -> Vec<ConnectionMsg<T::ProcessMsg, T::SystemMsgTypeParameter, T::ClientMsg>>;


pub struct Connection<T: ConnectionTypes> {
    pub pid: Pid,
    pub id: usize,
    pub state: T::State,
    pub sock: T::Socket,
    pub msg_writer: T::MsgWriter,
    pub msg_reader: T::MsgReader,
    pub system_envelope_callback: SystemEnvelopeCallback<T>,
    pub network_msg_callback: NetworkMsgCallback<T>,
    pub total_network_msgs_sent: usize,
    pub total_network_msgs_received: usize,
    pub total_system_envelopes_received: usize,
    pub total_system_requests_sent: usize
}

impl<T: ConnectionTypes> Connection<T> {
    pub fn new(pid: Pid, id: usize, socket: T::Socket) -> Connection<T> {
        Connection {
            pid: pid,
            id: id,
            state: T::State::new(),
            sock: socket,
            msg_writer: T::MsgWriter::new(),
            msg_reader: T::MsgReader::new(),
            system_envelope_callback: T::system_envelope_callback,
            network_msg_callback: T::network_msg_callback,
            total_network_msgs_sent: 0,
            total_network_msgs_received: 0,
            total_system_envelopes_received: 0,
            total_system_requests_sent: 0,
        }
    }
}
