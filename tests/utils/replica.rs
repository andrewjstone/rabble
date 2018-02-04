use rabble::{
    Pid,
    Process,
    Msg,
    Terminal
};

use super::messages::TestMsg;

/// A participant in chain replication
#[allow(dead_code)] // Not used in all tests
pub struct Replica {
    pid: Pid,
    next: Option<Pid>,
    history: Vec<usize>
}

#[allow(dead_code)] // Not used in all tests
impl Replica {
    pub fn new(pid: Pid, next: Option<Pid>) -> Replica {
        Replica {
            pid: pid,
            next: next,
            history: Vec::new()
        }
    }
}

impl<T> Process<TestMsg, T> for Replica where T: Terminal<TestMsg> {
    fn handle(&mut self,
              msg: Msg<TestMsg>,
              from: Pid,
              terminal: &mut T)
    {
        match msg {
            Msg::User(TestMsg::Op(val)) => {
                // We are at the head of the chain
                let to = self.next.as_ref().unwrap().clone();
                let msg = TestMsg::ForwardOp(val, from);
                terminal.send(to, msg);
                self.history.push(val);
            }
            Msg::User(TestMsg::ForwardOp(val, client)) => {
                self.history.push(val);
                if let Some(to) = self.next.as_ref() {
                    terminal.send(to.clone(), TestMsg::ForwardOp(val, client));
                } else {
                    terminal.send(client, TestMsg::OpComplete);
                }
            }
            Msg::User(TestMsg::GetHistory) => {
                terminal.send(from, TestMsg::History(self.history.clone()));
            },
            _ => ()
        }
    }
}

