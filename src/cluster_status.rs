use std::collections::HashSet;
use node_id::NodeId;
use members::Members;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct ClusterStatus {
    pub members: Members,
    pub connected: HashSet<NodeId>
}
