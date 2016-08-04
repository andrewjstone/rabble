use rustc_serialize::{Encodable, Decodable};
use pid::Pid;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Envelope<T: Encodable + Decodable> {
    pub to: Pid,
    pub from: Pid,
    pub msg: T
}
