use std::collections::HashSet;
use node_id::NodeId;
use members::Members;

pub struct ClusterStatus {
    pub members: Members,
    pub connected: HashSet<NodeId>
}
