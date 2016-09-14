use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use orset::ORSet;
use envelope::ProcessEnvelope;

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ExternalMsg<T: Encodable + Decodable> {
   Members {from: NodeId, orset: ORSet<NodeId>},
   Ping,
   User(ProcessEnvelope<T>)
}
