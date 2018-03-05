use std::fmt::{self, Debug, Formatter};
use orset::{ORSet, Delta};
use node_id::NodeId;
use envelope::Envelope;
use super::ClusterStatus;
use pid::Pid;
use process::Process;
use channel::Sender;

/// Messages sent to the Cluster Server
pub enum ClusterMsg<T> {
    Join(NodeId),
    Leave(NodeId),
    Envelope(Envelope<T>),
    GetStatus(Box<Sender<ClusterStatus>>),
    Spawn(Pid, Box<Process<T>>),
    Stop(Pid),
    RegisterService(Pid, Box<Sender<Envelope<T>>>),
    Shutdown,
}

impl<T> Debug for ClusterMsg<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ClusterMsg::GetStatus(_) => write!(f, "GetStatus"),
            ClusterMsg::Spawn(ref pid, _) => write!(f, "Spawn({})", pid),
            ClusterMsg::Stop(ref pid) => write!(f, "Stop({})", pid),
            ClusterMsg::RegisterService(ref pid, _) => write!(f, "RegisterService({})", pid),
            ref msg => write!(f, "{:?}", msg)
        }
    }
}

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExternalMsg<T> {
   Members {from: NodeId, orset: ORSet<NodeId>},
   Ping,
   Envelope(Envelope<T>),
   Delta(Delta<NodeId>)
}
