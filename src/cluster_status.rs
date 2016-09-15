use std::collections::HashSet;
use node_id::NodeId;
use members::Members;

#[derive(Clone, Debug)]
pub struct ClusterStatus {
    pub members: Members,
    pub connected: HashSet<NodeId>
}
