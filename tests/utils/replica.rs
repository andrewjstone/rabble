use rabble::{
    Pid,
    Process,
    Envelope,
    Msg
};

use super::messages::RabbleUserMsg;

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

impl Process<RabbleUserMsg> for Replica {
    fn handle(&mut self,
              msg: Msg<RabbleUserMsg>,
              from: Pid,
              output: &mut Vec<Envelope<RabbleUserMsg>>)
    {
        match msg {
            Msg::User(RabbleUserMsg::Op(val)) => {
                let msg = Msg::User(RabbleUserMsg::OpComplete);
                let reply = Envelope::new(from.clone(), self.pid.clone(), msg);

                // If there is no next pid send the reply to the original sender in the `from'
                // field. Otherwise forward to the next process in the chain.
                let envelope = self.next.as_ref().map_or(reply, |to| {
                    let msg = Msg::User(RabbleUserMsg::Op(val));
                    Envelope::new(to.clone(), from, msg)
                });

                self.history.push(val);
                output.push(envelope);
            },
            Msg::User(RabbleUserMsg::GetHistory) => {
                let msg = Msg::User(RabbleUserMsg::History(self.history.clone()));
                let envelope = Envelope::new(from, self.pid.clone(), msg);
                output.push(envelope);
            },
            _ => ()
        }
    }
}

