use process::Process;
use pid::Pid;
use amy;
use user_msg::UserMsg;
use envelope::Envelope;

pub enum ExecutorMsg<T: UserMsg> {
    Start(Pid, Box<Process<T>>),
    Stop(Pid),
    Envelope(Envelope<T>),
    RegisterService(Pid, amy::Sender<Envelope<T>>),
    Shutdown,
    Tick
}
