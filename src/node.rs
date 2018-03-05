use std::fmt::Debug;
use slog;
use amy::Sender;
use serde::{Serialize, Deserialize};
use node_id::NodeId;
use cluster::{ClusterMsg, ClusterStatus};
use pid::Pid;
use process::Process;
use envelope::Envelope;
use errors::*;
use channel;

macro_rules! send {
    ($s:ident.$t:ident, $msg:expr, $pid:expr, $errmsg:expr) => {
        if let Err(_) = $s.$t.send($msg) {
            return Err(ErrorKind::SendError($errmsg, $pid).into())
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
pub struct Node<T> {
    pub id: NodeId,
    pub logger: slog::Logger,
    cluster_tx: Sender<ClusterMsg<T>>
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone> Node<T> {
    /// Create a new node. This function should not be called by the user directly. It is called by
    /// by the user call to `rabble::rouse(..)` that initializes a rabble system for a single node.
    pub fn new(id: NodeId,
               cluster_tx: Sender<ClusterMsg<T>>,
               logger: slog::Logger) -> Node<T> {
        Node {
            id: id,
            cluster_tx: cluster_tx,
            logger: logger
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
              None,
              format!("ClusterMsg::Join({:?})", *node_id))
    }

    pub fn leave(&self, node_id: &NodeId) -> Result<()> {
        send!(self.cluster_tx,
              ClusterMsg::Leave(node_id.clone()),
              None,
              format!("ClusterMsg::Leave({:?})", *node_id))
    }

    /// Add a process to the executor that can be sent Envelopes addressed to its pid
    pub fn spawn(&self, pid: &Pid, process: Box<Process<T>>) -> Result<()> {
        send!(self.cluster_tx,
              ClusterMsg::Spawn(pid.clone(), process),
              Some(pid.clone()),
              format!("ClusterMsg::Spawn({}, ..)", pid))
    }

    /// Remove a process from the executor
    pub fn stop(&self, pid: &Pid) -> Result<()> {
        send!(self.cluster_tx,
              ClusterMsg::Stop(pid.clone()),
              Some(pid.clone()),
              format!("ClusterMsg::Stop({}, ..)", pid))
    }

    /// Register a Service's sender with the executor so that it can be sent messages addressed to
    /// its pid
    pub fn register_service(&self, pid: &Pid, tx: Box<channel::Sender<Envelope<T>>>) -> Result<()>
    {
        send!(self.cluster_tx,
              ClusterMsg::RegisterService(pid.clone(), tx),
              Some(pid.clone()),
              format!("ClusterMsg::RegisterService({}, ..)", pid))
    }

    /// Send an envelope to the executor so it gets routed to the appropriate process or service
    pub fn send(&self, envelope: Envelope<T>) -> Result<()> {
        let to = envelope.to.clone();
        send!(self.cluster_tx,
              ClusterMsg::Envelope(envelope),
              Some(to),
              "ClusterMsg::Envelope(envelope)".to_string())
    }

    /// Get the status of the cluster server
    pub fn cluster_status(&self, reply_tx: Box<channel::Sender<ClusterStatus>>) -> Result<()> {
        self.cluster_tx.send(ClusterMsg::GetStatus(reply_tx)).map_err(|_| {
            let errmsg = "ClusterMsg::GetStatus".to_owned();
            ErrorKind::SendError(errmsg, None).into()
        })
    }

    /// Shutdown the node
    pub fn shutdown(&self) {
        let _ = self.cluster_tx.send(ClusterMsg::Shutdown);
    }
}
