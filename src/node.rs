use std::sync::mpsc::{Sender, SendError};
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use pid::Pid;
use correlation_id::CorrelationId;

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

    pub fn send(&self, msg: ExecutorMsg<T, U>) -> Result<(), SendError<ExecutorMsg<T, U>>> {
        self.executor_tx.send(msg)
    }

    pub fn executor_status(&self, from: Pid, correlation_id: Option<CorrelationId>)
        -> Result<(), SendError<ExecutorMsg<T, U>>>
    {
            self.executor_tx.send(ExecutorMsg::GetStatus(from, correlation_id))
    }

    pub fn cluster_status(&self, from: Pid, correlation_id: Option<CorrelationId>)
        -> Result<(), SendError<ClusterMsg<T>>>
    {
        self.cluster_tx.send(ClusterMsg::GetStatus(from, correlation_id))
    }
}
