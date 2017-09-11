use amy::Notification;
use orset::{ORSet, Delta};
use node_id::NodeId;
use envelope::Envelope;
use correlation_id::CorrelationId;

/// Messages sent to the Cluster Server
pub enum ClusterMsg {
    PollNotifications(Vec<Notification>),
    Join(NodeId),
    Leave(NodeId),
    Envelope(Envelope),
    GetStatus(CorrelationId),
    Shutdown
}

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExternalMsg {
   Members {from: NodeId, orset: ORSet<NodeId>},
   Ping,
   Envelope(Envelope),
   Delta(Delta<NodeId>)
}
