use std::io::{Read, Write};
use errors::*;

/// This trait provides for reading framed messages from a `Read` type, decoding them and
/// returning them. It buffers incomplete messages. Reading of only a single message of a time is to
/// allow for strategies that prevent starvation of other readers.
///
/// This trait provides for serializing and framing messages, and then writing them to a `Write`
/// type. When a complete message cannot be sent it is buffered for when the `Write` type is next
/// writable.
///
/// We write all possible data to the writer until it blocks or there is no more data to be written.
/// Since all output is in response to input, we don't worry about starvation of writers. In order
/// to minimize memory consumption we just write as much as possible and worry about starvation
/// management on the reader side.
pub trait Protocol {
    type Msg;

    fn new() -> Self;

    /// Read and decode a single message at a time.
    ///
    /// This function should be called until it returns Ok(None) in which case there is no more data
    /// left to return. For async sockets this signals that the socket should be re-registered.
    fn read_msg<T: Read>(&mut self, reader: &mut T) -> Result<Option<Self::Msg>>;

    /// Write out as much pending data as possible. Append `msg` to the pending data if not `None`.
    fn write_msgs<T: Write>(&mut self, writer: &mut T, msg: Option<&Self::Msg>) -> Result<bool>;
}
