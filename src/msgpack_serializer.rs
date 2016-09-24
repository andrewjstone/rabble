use std::io::{Read, Write};
use std::marker::PhantomData;
use std::fmt::Debug;
use amy::{FrameReader, FrameWriter};
use msgpack::{Encoder, Decoder};
use rustc_serialize::{Encodable, Decodable};
use errors::*;
use serialize::Serialize;

const MAX_FRAME_SIZE: u32 = 64*1024*1024; // 64 MB

pub struct MsgpackSerializer<T: Encodable + Decodable + Debug> {
    frame_reader: FrameReader,
    frame_writer: FrameWriter,
    phantom: PhantomData<T>
}

impl<T: Encodable + Decodable + Debug> Serialize for MsgpackSerializer<T> {
    type Msg = T;

    fn new() -> MsgpackSerializer<T> {
        MsgpackSerializer {
            frame_reader: FrameReader::new(MAX_FRAME_SIZE),
            frame_writer: FrameWriter::new(),
            phantom: PhantomData
        }
    }

    fn read_msg<U: Read>(&mut self, reader: &mut U) -> Result<Option<T>> {
        try!(self.frame_reader.read(reader).chain_err(|| "Failed to read from socket"));
        self.frame_reader.iter_mut().next().map_or(Ok(None), |frame| {
            let mut decoder = Decoder::new(&frame[..]);
            let msg = try!(Decodable::decode(&mut decoder).chain_err(|| "Failed to decode msgpack frame"));
            Ok(Some(msg))
        })
    }

    fn write_msgs<U: Write>(&mut self, writer: &mut U, msg: Option<&T>) -> Result<bool> {
        if msg.is_none() {
            return self.frame_writer.write(writer, None)
                .chain_err(|| "Failed to write encoded message")
        }

        let mut encoded = Vec::new();
        try!(msg.as_ref().unwrap().encode(&mut Encoder::new(&mut encoded))
             .chain_err(|| format!("Failed to encode message {:?}", msg)));
        self.frame_writer.write(writer, Some(encoded))
            .chain_err(|| "Failed to write encoded message")
    }
}
