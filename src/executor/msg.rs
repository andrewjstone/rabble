use envelope::Envelope;
use process::Process;
use pid::Pid;
use correlation_id::CorrelationId;
use amy;

pub enum ExecutorMsg<T> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterService(Pid, amy::Sender<Envelope<T>>),
    GetStatus(CorrelationId),
    Shutdown,
    Tick
}
