use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use envelope::Envelope;
use correlation_id::CorrelationId;

pub trait Process<T: Encodable + Decodable, U> : Send {
    fn handle(&mut self, msg: T,
              from: Pid,
              correlation_id: Option<CorrelationId>) -> &mut Vec<Envelope<T, U>>;
}
