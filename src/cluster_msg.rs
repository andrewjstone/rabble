use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use amy::Notification;
use node_id::NodeId;
use envelope::Envelope;
use pid::Pid;
use correlation_id::CorrelationId;

/// Messages sent to the Cluster Server
pub enum ClusterMsg<T: Encodable + Decodable + Debug + Clone> {
    PollNotifications(Vec<Notification>),
    Join(NodeId),
    Envelope(Envelope<T>),
    GetStatus(CorrelationId),
    Shutdown
}
