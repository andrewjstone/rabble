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

macro_rules! send {
    ($s:ident.$t:ident, $msg:expr, $errmsg:expr) => {
        if let Err(_) = $s.$t.send($msg) {
            return Err(ErrorKind::SendError($errmsg).into())
        } else {
            return Ok(());
        }
    }
}

/// A Node represents a way for services to interact with rabble internals.
///
/// The Node api is used by services and their handlers to send messages, get status, join
/// nodes into a cluster, etc...
#[derive(Clone)]
pub struct Node<T: Encodable + Decodable, U: Debug + Clone> {
    pub id: NodeId,
    executor_tx: Sender<ExecutorMsg<T, U>>,
    cluster_tx: Sender<ClusterMsg<T>>
}

impl<T: Encodable + Decodable, U: Debug + Clone> Node<T, U> {
    /// Create a new node. This function should not be called by the user directly. It is called by
    /// by the user call to `rabble::rouse(..)` that initializes a rabble system for a single node.
    pub fn new(id: NodeId,
               executor_tx: Sender<ExecutorMsg<T, U>>,
               cluster_tx: Sender<ClusterMsg<T>>) -> Node<T, U> {
        Node {
            id: id,
            executor_tx: executor_tx,
            cluster_tx: cluster_tx
        }
    }

    /// Join 1 node to another to form a cluster.
    ///
    /// Node joins are transitive such that if `Node A` joins `Node B` which is already joined with
    /// `Node C`, then `Node A` will become connected to both `Node B` and `Node C`.
    ///
    /// Join's are not immediate. The local member state is updated and the joining node will
    /// continuously try to connect to the remote node so that they can exchange membership
    /// information and participate in peer operations.
    pub fn join(&self, node_id: &NodeId) -> Result<()> {
        send!(self.cluster_tx,
              ClusterMsg::Join(node_id.clone()),
              format!("ClusterMsg::Join({:?})", node_id).to_string())
    }

    /// Add a process to the executor that an be sent ProcessEnvelopes addressed to its pid
    pub fn spawn(&self, pid: &Pid, process: Box<Process<Msg=T, SystemUserMsg=U>>) -> Result<()> {
        send!(self.executor_tx,
              ExecutorMsg::Start(pid.clone(), process),
              format!("ExecutorMsg::Start({}, ..)", pid))
    }

    /// Register a Service's sender with the executor so that it can be sent messages addressed to
    /// its pid
    pub fn register_system_thread(&self, pid: &Pid, tx: &amy::Sender<SystemEnvelope<U>>) -> Result<()>
    {
        send!(self.executor_tx,
              ExecutorMsg::RegisterSystemThread(pid.clone(), tx.clone()),
              format!("ExecutorMsg::RegisterSystemThread({}, ..)", pid))
    }

    /// Send an envelope to the executor so it gets routed to the appropriate process or system
    /// thread
    pub fn send(&self, envelope: Envelope<T, U>) -> Result<()> {
        send!(self.executor_tx, ExecutorMsg::User(envelope), "ExecutorMsg::User(envelope)".to_string())
    }

    /// Get the status of the executor
    pub fn executor_status(&self, correlation_id: CorrelationId) -> Result<()> {
        send!(self.executor_tx,
              ExecutorMsg::GetStatus(correlation_id),
              "ExecutorMsg::GetStatus".to_string())
    }

    /// Get the status of the cluster server
    pub fn cluster_status(&self, correlation_id: CorrelationId) -> Result<()> {
        send!(self.cluster_tx,
              ClusterMsg::GetStatus(correlation_id),
              "ClusterMsg::GetStatus".to_string())
    }

    /// Shutdown the node
    pub fn shutdown(&self) {
        self.executor_tx.send(ExecutorMsg::Shutdown);
        self.cluster_tx.send(ClusterMsg::Shutdown);
    }
}
