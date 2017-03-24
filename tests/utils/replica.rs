use rabble::{
    Pid,
    Process,
    Envelope,
    CorrelationId,
    Msg
};

use super::messages::RabbleUserMsg;

/// A participant in chain replication
#[allow(dead_code)] // Not used in all tests
pub struct Replica {
    pid: Pid,
    next: Option<Pid>,
    history: Vec<u64>,
    output: Vec<Envelope<RabbleUserMsg>>
}

#[allow(dead_code)] // Not used in all tests
impl Replica {
    pub fn new(pid: Pid, next: Option<Pid>) -> Replica {
        Replica {
            pid: pid,
            next: next,
            history: Vec::new(),
            output: Vec::with_capacity(1)
        }
    }
}

impl Process<RabbleUserMsg> for Replica {

    fn handle(&mut self,
              msg: Msg<RabbleUserMsg>,
              _from: Pid,
              correlation_id: CorrelationId)
        -> &mut Vec<Envelope<RabbleUserMsg>>
    {
        let to = correlation_id.pid.clone();
        let from = self.pid.clone();
        match msg {
            Msg::User(RabbleUserMsg::Op(val)) => {
                let msg = Msg::User(RabbleUserMsg::OpComplete);
                let reply = Envelope {
                    to: to,
                    from: from,
                    msg: msg,
                    correlation_id: correlation_id.clone()
                };

                // If there is no next pid send the reply to the original caller in the correlation
                // id. Otherwise forward to the next process in the chain.
                let envelope = self.next.as_ref().map_or(reply, |to| {
                    let from = self.pid.clone();
                    let msg = Msg::User(RabbleUserMsg::Op(val));
                    Envelope {
                        to: to.clone(),
                        from: from,
                        msg: msg,
                        correlation_id: correlation_id
                    }
                });

                self.history.push(val);
                self.output.push(envelope);
            },
            Msg::User(RabbleUserMsg::GetHistory) => {
                let msg = Msg::User(RabbleUserMsg::History(self.history.clone()));
                let envelope = Envelope {
                    to: to,
                    from: from,
                    msg: msg,
                    correlation_id: correlation_id
                };
                self.output.push(envelope);
            },
            _ => ()
        }
        &mut self.output
    }
}

