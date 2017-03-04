use std::collections::HashSet;
use node_id::NodeId;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct ClusterStatus {
    pub members: HashSet<NodeId>,
    pub established: HashSet<NodeId>,
    pub num_connections: usize
}
