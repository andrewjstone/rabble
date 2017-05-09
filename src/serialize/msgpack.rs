use std::io::{Read, Write};
use std::marker::PhantomData;
use std::fmt::Debug;
use amy::{FrameReader, FrameWriter};
use msgpack::{Serializer, Deserializer};
use serde::{Serialize, Deserialize};
use errors::*;
use serialize;

const MAX_FRAME_SIZE: u32 = 64*1024*1024; // 64 MB

pub struct MsgpackSerializer<T> {
    frame_reader: FrameReader,
    frame_writer: FrameWriter,
    phantom: PhantomData<T>
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone> serialize::Serialize for MsgpackSerializer<T> {
    type Msg = T;

    fn new() -> MsgpackSerializer<T> {
        MsgpackSerializer {
            frame_reader: FrameReader::new(MAX_FRAME_SIZE),
            frame_writer: FrameWriter::new(),
            phantom: PhantomData
        }
    }

    fn read_msg<U: Read>(&mut self, reader: &mut U) -> Result<Option<T>> {
        try!(self.frame_reader.read(reader).chain_err(|| "Msgpack Serializer failed to read from socket"));
        self.frame_reader.iter_mut().next().map_or(Ok(None), |frame| {

            let mut deserializer = Deserializer::new(&frame[..]);
            let msg = try!(Deserialize::deserialize(&mut deserializer)
                           .chain_err(|| "Failed to decode msgpack frame"));
            Ok(Some(msg))
        })
    }

    fn write_msgs<U: Write>(&mut self, writer: &mut U, msg: Option<&T>) -> Result<bool> {
        if msg.is_none() {
            return self.frame_writer.write(writer, None)
                .chain_err(|| "Failed to write encoded message")
        }

        let mut encoded = Vec::new();
        try!(msg.as_ref().unwrap().serialize(&mut Serializer::new(&mut encoded))
             .chain_err(|| format!("Failed to encode message {:?}", msg)));
        self.frame_writer.write(writer, Some(encoded))
            .chain_err(|| "Failed to write encoded message")
    }

    fn set_writable(&mut self) {
        self.frame_writer.writable();
    }

    fn is_writable(&self) -> bool {
        self.frame_writer.is_writable()
    }
}
