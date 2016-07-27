use rustc_serialize::{Encodable, Decodable};
use node::Node;
use orset::ORSet;

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ExternalMsg<T: Encodable + Decodable> {
   Members {from: Node, orset: ORSet<Node>},
   Ping,
   User(T)
}
