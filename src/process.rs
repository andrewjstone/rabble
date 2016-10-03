use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use pid::Pid;
use msg::Msg;
use envelope::Envelope;
use correlation_id::CorrelationId;

pub trait Process : Send {
    type Msg: Encodable + Decodable + Debug + Clone;

    fn handle(&mut self,
              msg: Msg<Self::Msg>,
              from: Pid,
              correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<Self::Msg>>;
}
