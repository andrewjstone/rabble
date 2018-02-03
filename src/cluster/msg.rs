use std::time::Duration;
use amy::Notification;
use orset::{ORSet, Delta};
use node_id::NodeId;
use envelope::Envelope;
use super::ClusterStatus;
use futures::sync::oneshot::Sender;
use terminal::TimerId;
use pid::Pid;

/// Messages sent to the Cluster Server
pub enum ClusterMsg<T> {
    PollNotifications(Vec<Notification>),
    Join(NodeId),
    Leave(NodeId),
    Envelope(Envelope<T>),
    GetStatus(Sender<ClusterStatus>),
    Shutdown,

    // Currently the executor uses the cluster server to manage timers
    // This simplifies the implementation of the executor since it doesn't need to use amy channels
    // that can wakeup with timer notifications or handle `Tick` messages every 10ms, and maintain
    // it's own timer wheel to process them. It only receives timeouts as envelopes destined for
    // their specific processes. This may not result in less messages throughout the system in total
    // (depending upon number of timeouts registered, cancellation rate, etc...) but it likely will
    // result in less wakeups of the executor thread, since it won't get a tick every 10ms
    // regardless of whether it actually needs to process a timeout.
    StartTimer(Pid, TimerId, Duration),
    CancelTimer(Pid, TimerId)
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
