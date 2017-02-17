use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use correlation_id::CorrelationId;
use status::StatusTable;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum Msg<T: Encodable + Decodable + Debug + Clone> {
    User(T),
    Status(StatusTable),
    StartTimer(usize), // time in ms
    CancelTimer(Option<CorrelationId>),
    Timeout,
    Shutdown
}
