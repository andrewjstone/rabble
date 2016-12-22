use std::io::{Read, Write};
use std::marker::PhantomData;
use amy::{FrameReader, FrameWriter};
use protobuf::{Message, MessageStatic, parse_from_bytes};
use errors::*;
use serialize::Serialize;

const MAX_FRAME_SIZE: u32 = 64*1024*1024; // 64 MB

pub struct ProtobufSerializer<M: Message + MessageStatic> {
    frame_reader: FrameReader,
    frame_writer: FrameWriter,
    phantom: PhantomData<M>
}

impl<M: Message + MessageStatic> Serialize for ProtobufSerializer<M> {
    type Msg = M;

    fn new() -> ProtobufSerializer<M> {
        ProtobufSerializer {
            frame_reader: FrameReader::new(MAX_FRAME_SIZE),
            frame_writer: FrameWriter::new(),
            phantom: PhantomData
        }
    }

    fn read_msg<U: Read>(&mut self, reader: &mut U) -> Result<Option<M>> {
        try!(self.frame_reader.read(reader).chain_err(|| "Serializer failed to read from socket"));
        self.frame_reader.iter_mut().next().map_or(Ok(None), |frame| {
            let msg: M = try!(parse_from_bytes(&frame[..]));
            Ok(Some(msg))
        })
    }

    fn write_msgs<U: Write>(&mut self, writer: &mut U, msg: Option<&M>) -> Result<bool> {
        if msg.is_none() {
            return self.frame_writer.write(writer, None)
                .chain_err(|| "Failed to write encoded message")
        }
        let encoded = try!(msg.as_ref().unwrap().write_to_bytes());
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
