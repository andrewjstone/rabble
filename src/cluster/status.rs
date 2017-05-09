use std::collections::HashSet;
use node_id::NodeId;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub members: HashSet<NodeId>,
    pub established: HashSet<NodeId>,
    pub num_connections: usize
}
