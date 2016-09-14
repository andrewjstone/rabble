use rustc_serialize::{Encodable, Decodable};
use amy::Notification;
use node_id::NodeId;
use envelope::ProcessEnvelope;
use pid::Pid;

pub type CorrelationId = usize;

/// Messages sent to the Cluster Server
pub enum ClusterMsg<T: Encodable + Decodable> {
    PollNotifications(Vec<Notification>),
    Join(NodeId),
    User(ProcessEnvelope<T>),
    GetStatus(Pid, CorrelationId)
}
