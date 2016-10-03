use rabble::{
    Pid,
    Process,
    Envelope,
    CorrelationId,
    Msg
};

use super::messages::RabbleUserMsg;

/// A participant in chain replication
pub struct Replica {
    pid: Pid,
    next: Option<Pid>,
    history: Vec<usize>,
    output: Vec<Envelope<RabbleUserMsg>>
}

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

impl Process for Replica {
    type Msg = RabbleUserMsg;

    fn handle(&mut self,
              msg: Msg<RabbleUserMsg>,
              _from: Pid,
              correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<RabbleUserMsg>>
    {
        let to = correlation_id.as_ref().unwrap().pid.clone();
        let from = self.pid.clone();
        match msg {
            Msg::User(RabbleUserMsg::Op(val)) => {
                // This doesn't work. Replies don't go back to the pid on a diff node.
                let msg = Msg::User(RabbleUserMsg::OpComplete);
                let reply = Envelope::new(to, from, msg, correlation_id.clone());

                // If there is no next pid send the reply to the original caller in the correlation
                // id. Otherwise forward to the next process in the chain.
                let envelope = self.next.as_ref().map_or(reply, |to| {
                    let from = self.pid.clone();
                    let msg = Msg::User(RabbleUserMsg::Op(val));
                    Envelope::new(to.clone(), from, msg, correlation_id)
                });

                self.history.push(val);
                self.output.push(envelope);
            },
            Msg::User(RabbleUserMsg::GetHistory) => {
                let msg = Msg::User(RabbleUserMsg::History(self.history.clone()));
                let envelope = Envelope::new(to, from, msg, correlation_id);
                self.output.push(envelope);
            },
            _ => ()
        }
        &mut self.output
    }
}

