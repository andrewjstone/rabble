use envelope::Envelope;
use process::Process;
use pid::Pid;
use std::sync::mpsc::Sender;
use futures::sync::oneshot;
use super::ExecutorStatus;

pub enum ExecutorMsg<T> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterService(Pid, Sender<Envelope<T>>),
    GetStatus(oneshot::Sender<ExecutorStatus>),
    Shutdown
}
