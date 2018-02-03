use pid::Pid;
use msg::Msg;
use terminal::Terminal;
use executor::ExecutorTerminal;

pub trait Process<M, T=ExecutorTerminal<M>> : Send where T: Terminal<M> {
    /// Initialize process state if necessary
    fn init(&mut self, _terminal: &mut T) {}

    /// Handle messages from other actors
    fn handle(&mut self,
              msg: Msg<M>,
              from: Pid,
              terminal: &mut T);
}
