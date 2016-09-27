use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use envelope::Envelope;
use correlation_id::CorrelationId;

pub trait Process : Send {
    type Msg: Encodable + Decodable;
    type SystemUserMsg: Debug;

    fn handle(&mut self,
              msg: Self::Msg,
              from: Pid,
              correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<Self::Msg, Self::SystemUserMsg>>;
}
