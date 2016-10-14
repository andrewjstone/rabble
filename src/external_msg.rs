use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use node_id::NodeId;
use orset::{ORSet, Delta};
use envelope::Envelope;

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone, RustcEncodable, RustcDecodable)]
pub enum ExternalMsg<T: Encodable + Decodable + Debug + Clone> {
   Members {from: NodeId, orset: ORSet<NodeId>},
   Ping,
   Envelope(Envelope<T>),
   Delta(Delta<NodeId>)
}
