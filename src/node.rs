use std::sync::mpsc::Sender;
use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use service::Service;
use pid::Pid;

#[derive(Clone)]
pub struct Node<T: Encodable + Decodable, U> {
    pub id: NodeId,
    executor_tx: Sender<ExecutorMsg<T, U>>,
    cluster_tx: Sender<ClusterMsg<T>>
}

impl<T: Encodable + Decodable, U> Node<T, U> {
    pub fn new(id: NodeId,
               executor_tx: Sender<ExecutorMsg<T, U>>,
               cluster_tx: Sender<ClusterMsg<T>>) -> Node<T, U> {
        Node {
            id: id,
            executor_tx: executor_tx,
            cluster_tx: cluster_tx
        }
    }

    pub fn register_service(&self, name: &str) -> Service<T, U> {
        let pid = Pid {
            name: name.to_string(),
            group: Some("Service".to_string()),
            node: self.id.clone()
        };
        Service::new(pid, self.executor_tx.clone(), self.cluster_tx.clone())
    }
}
