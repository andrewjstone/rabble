use std::sync::mpsc::Sender;
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use envelope::Envelope;
use process::Process;
use pid::Pid;
use correlation_id::CorrelationId;
use amy;

pub enum ExecutorMsg<T: Encodable + Decodable + Debug + Clone> {
    Start(Pid, Box<Process<Msg=T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterSystemThread(Pid, amy::Sender<Envelope<T>>),
    GetStatus(CorrelationId),
    Shutdown
}
