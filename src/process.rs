use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use envelope::Envelope;

pub trait Process<T: Encodable + Decodable, U> : Send {
    fn handle(&mut self, msg: T, from: Pid) -> &mut Vec<Envelope<T, U>>;
}
