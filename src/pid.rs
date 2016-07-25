use std::fmt::{Display, Error, Formatter};
use node::Node;

/// A globally unique process id
///
/// Pids can be grouped together for various reasons. This grouping acts like a namespace. If
/// a Process is not a member of a group, the `group` member of the Pid will be `None`.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, RustcEncodable, RustcDecodable)]
pub struct Pid {
    pub group: Option<String>,
    pub name: String,
    pub node: Node,
}

impl Display for Pid {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self.group {
            None => write!(f, "{}::{}::{}", self.name, self.node.name, self.node.addr),
            Some(ref g) => write!(f, "{}::{}::{}::{}", g, self.name, self.node.name, self.node.addr)
        }
    }
}
