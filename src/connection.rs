use std::io::{self, Read, Write};
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use node::Node;
use envelope::SystemEnvelope;

/// The result of calling MsgWriter::write_msg()
pub enum WriteResult {
    /// The writer needs to be re-registered with the poller
    WouldBlock,
    /// There are no more messages to send
    EmptyBuffer,
    /// There are more messages to send
    MoreMessagesInBuffer,
    /// There was an io error with a kind other than `io::ErrorKind::WouldBlock`
    Err(io::Error)
}

/// This trait provides for reading framed messages from a `Read` type, decoding them and
/// returning them. It buffers incomplete messages. Reading of only a single message of a time is to
/// allow for strategies that prevent starvation of other readers.
pub trait MsgReader {
    type Msg;
    type Buffer;

    /// Create a new MsgReader
    fn new() -> Self;

    /// Read and decode a single message at a time.
    ///
    /// This function should be called until it returns Ok(None) in which case there is no more data
    /// left to return. For async sockets this signals that the socket should be re-registered.
    fn read_msg<T: Read>(&mut self, reader: &mut T) -> io::Result<Option<Self::Msg>>;
}

/// This trait provides for serializing and framing messages, and then writing them to a `Write`
/// type. When a complete message cannot be sent it is buffered for when the `Write` type is next
/// writable. Writing of only a single message at a time is to allow for strategies that prevent
/// starvation of other writers.
pub trait MsgWriter {
    type Msg;
    type Buffer;

    /// Create a new MsgWriter
    fn new() -> Self;

    /// Complete the write of a single message that's already in the buffer or the message passed in
    /// if no other messages are buffered.
    ///
    /// If there are still messages in the buffer to write, but no new messages, this function can
    /// be called with `msg = None`.
    fn write_msg<T: Write>(&mut self, writer: &mut T, msg: Option<Self::Msg>) -> WriteResult;
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
    type Socket: Read + Write;
    type ProcessMsg: Encodable + Decodable;
    type SystemMsgTypeParameter: Debug + Clone;
    type MsgWriter: MsgWriter + 'static;
    type MsgReader: MsgReader + 'static;

    fn system_envelope_callback(Self::State,
                                &Node<Self::ProcessMsg, Self::SystemMsgTypeParameter>,
                                SystemEnvelope<Self::SystemMsgTypeParameter>)
        -> Vec<<<Self as ConnectionTypes>::MsgWriter as MsgWriter>::Msg>;

    fn network_msg_callback(Self::State,
                            &Node<Self::ProcessMsg, Self::SystemMsgTypeParameter>,
                            <<Self as ConnectionTypes>::MsgReader as MsgReader>::Msg)
        -> Vec<<<Self as ConnectionTypes>::MsgWriter as MsgWriter>::Msg>;
}

type SystemEnvelopeCallback<T: ConnectionTypes> =
    fn(T::State,
       &Node<T::ProcessMsg, T::SystemMsgTypeParameter>,
       SystemEnvelope<T::SystemMsgTypeParameter>)
  -> Vec<<<T as ConnectionTypes>::MsgWriter as MsgWriter>::Msg>;

type NetworkMsgCallback<T: ConnectionTypes> =
    fn(T::State,
       &Node<T::ProcessMsg, T::SystemMsgTypeParameter>,
       <<T as ConnectionTypes>::MsgReader as MsgReader>::Msg)
  -> Vec<<<T as ConnectionTypes>::MsgWriter as MsgWriter>::Msg>;


pub struct Connection<T: ConnectionTypes> {
    id: usize,
    state: T::State,
    sock: T::Socket,
    msg_writer: T::MsgWriter,
    msg_reader: T::MsgReader,
    system_envelope_callback: SystemEnvelopeCallback<T>,
    network_msg_callback: NetworkMsgCallback<T>,
    total_network_msgs_sent: usize,
    total_network_msgs_received: usize,
    total_system_envelopes_received: usize,
    total_system_requests_sent: usize
}

impl<T: ConnectionTypes> Connection<T> {
    pub fn new(id: usize, socket: T::Socket) -> Connection<T> {
        Connection {
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
