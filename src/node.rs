use std::sync::mpsc::{Sender, SendError};
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use pid::Pid;
use correlation_id::CorrelationId;
use process::Process;
use envelope::{Envelope, SystemEnvelope};
use amy;
use errors::*;

macro_rules! send_to_executor {
    ($s:ident, $msg:expr, $errmsg:expr) => {
        if let Err(_) = $s.executor_tx.send($msg) {
            return Err(ErrorKind::SendError($errmsg).into())
        } else {
            return Ok(());
        }
    };
}

#[derive(Clone)]
pub struct Node<T: Encodable + Decodable, U: Debug + Clone> {
    pub id: NodeId,
    executor_tx: Sender<ExecutorMsg<T, U>>,
    cluster_tx: Sender<ClusterMsg<T>>
}

impl<T: Encodable + Decodable, U: Debug + Clone> Node<T, U> {
    pub fn new(id: NodeId,
               executor_tx: Sender<ExecutorMsg<T, U>>,
               cluster_tx: Sender<ClusterMsg<T>>) -> Node<T, U> {
        Node {
            id: id,
            executor_tx: executor_tx,
            cluster_tx: cluster_tx
        }
    }

    pub fn spawn(&self, pid: &Pid, process: Box<Process<Msg=T, SystemUserMsg=U>>) -> Result<()> {
        send_to_executor!(self,
                          ExecutorMsg::Start(pid.clone(), process),
                          format!("ExecutorMsg::Start({}, ..)", pid))
    }

    pub fn register_system_thread(&self, pid: &Pid, tx: &amy::Sender<SystemEnvelope<U>>) -> Result<()>
    {
        send_to_executor!(self,
                          ExecutorMsg::RegisterSystemThread(pid.clone(), tx.clone()),
                          format!("ExecutorMsg::RegisterSystemThread({}, ..)", pid))
    }

    pub fn send(&self, envelope: Envelope<T, U>) -> Result<()> {
        send_to_executor!(self,
                          ExecutorMsg::User(envelope),
                          "ExecutorMsg::User(envelope)".to_string())
    }

    pub fn executor_status(&self, from: Pid, correlation_id: Option<CorrelationId>) -> Result<()> {
            send_to_executor!(self,
                              ExecutorMsg::GetStatus(from, correlation_id),
                              "ExecutorMsg::GetStatus".to_string())
    }

    pub fn cluster_status(&self, from: Pid, correlation_id: Option<CorrelationId>) -> Result<()> {
        if let Err(e) = self.cluster_tx.send(ClusterMsg::GetStatus(from, correlation_id)) {
            return Err(ErrorKind::SendError("ClusterMsg::GetStatus".to_string()).into());
        }
        Ok(())
    }

    /// Shutdown the node
    pub fn shutdown(&self) {
        self.executor_tx.send(ExecutorMsg::Shutdown);
        self.cluster_tx.send(ClusterMsg::Shutdown);
    }
}
