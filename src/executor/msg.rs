use envelope::Envelope;
use process::Process;
use pid::Pid;
use correlation_id::CorrelationId;
use amy;

pub enum ExecutorMsg {
    Start(Pid, Box<Process>),
    Stop(Pid),
    Envelope(Envelope),
    RegisterService(Pid, amy::Sender<Envelope>),
    GetStatus(CorrelationId),
    Shutdown,
    Tick
}
