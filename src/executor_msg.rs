use std::sync::mpsc::Sender;
use rustc_serialize::{Encodable, Decodable};
use envelope::{Envelope, SystemEnvelope};
use process::Process;
use pid::Pid;
use correlation_id::CorrelationId;
use amy;

pub enum ExecutorMsg<T: Encodable + Decodable, U> {
    Start(Pid, Box<Process<T, U>>),
    Stop(Pid),
    User(Envelope<T, U>),
    RegisterSystemThread(Pid, amy::Sender<SystemEnvelope<U>>),
    GetStatus(Pid, Option<CorrelationId>)
}
