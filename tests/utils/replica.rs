use rabble::{
    Pid,
    Process,
    Envelope,
    CorrelationId,
    SystemMsg,
};

use super::messages::{ProcessMsg, SystemUserMsg};

/// A participant in chain replication
pub struct Replica {
    pid: Pid,
    next: Option<Pid>,
    history: Vec<usize>,
    output: Vec<Envelope<ProcessMsg, SystemUserMsg>>
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
    type Msg = ProcessMsg;
    type SystemUserMsg = SystemUserMsg;

    fn handle(&mut self,
              msg: ProcessMsg,
              _from: Pid,
              correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<ProcessMsg, SystemUserMsg>>
    {
        let to = correlation_id.as_ref().unwrap().pid.clone();
        let from = self.pid.clone();
        match msg {
            ProcessMsg::Op(val) => {
                let msg = SystemMsg::User(SystemUserMsg::OpComplete);
                let reply = Envelope::new_system(to, from, msg, correlation_id.clone());

                // If there is no next pid send the reply to the original caller in the correlation
                // id. Otherwise forward to the next process in the chain.
                let envelope = self.next.as_ref().map_or(reply, |to| {
                    let from = self.pid.clone();
                    Envelope::new_process(to.clone(), from, ProcessMsg::Op(val), correlation_id)
                });

                self.history.push(val);
                self.output.push(envelope);
            },
            ProcessMsg::GetHistory => {
                let msg = SystemMsg::User(SystemUserMsg::History(self.history.clone()));
                let envelope = Envelope::new_system(to, from, msg, correlation_id);
                self.output.push(envelope);
            }
        }
        &mut self.output
    }
}

