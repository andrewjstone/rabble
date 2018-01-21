use envelope::Envelope;
use process::Process;
use pid::Pid;
use futures::sync::oneshot;
use channel;
use super::ExecutorStatus;

pub enum ExecutorMsg<T> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterService(Pid, Box<channel::Sender<Envelope<T>>>),
    GetStatus(oneshot::Sender<ExecutorStatus>),
    Shutdown
}
