use envelope::Envelope;
use process::Process;
use pid::Pid;
use channel::Sender;
use super::ExecutorStatus;

pub enum ExecutorMsg<T> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterService(Pid, Box<Sender<Envelope<T>>>),
    GetStatus(Box<Sender<ExecutorStatus>>),
    Shutdown
}
