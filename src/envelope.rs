use pid::Pid;

pub struct Envelope<T> {
    pub to: Pid,
    pub from: Pid,
    pub msg: T
}
