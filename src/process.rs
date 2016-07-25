use pid::Pid;
use envelope::Envelope;

pub trait Process<T> {
    fn handle(&mut self, msg: T, from: Pid) -> &mut Vec<Envelope<T>>;
}
