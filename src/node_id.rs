use std::fmt::{Display, Error, Formatter};
use std::str::FromStr;
use std::convert::From;
use pb_messages;

// RustcEncodable/RustcDecodable still required because of ORSet usage and also so that
// MsgpackSerializer can use NodeId
#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, RustcEncodable, RustcDecodable)]
pub struct NodeId {
    pub name: String,
    pub addr: String
}

impl Display for NodeId {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        try!(fmt.write_fmt(format_args!("{}@{}", self.name, self.addr)));
        Ok(())
    }
}

impl FromStr for NodeId {
    type Err = String;

    fn from_str(s: &str) -> Result<NodeId, String> {
        let v: Vec<&str> = s.split("@").collect();
        if v.len() != 2 {
            return Err("Invalid Pid format - Must be of form 'name@addr'".to_string())
        }
        Ok(NodeId {
            name: v[0].to_string(),
            addr: v[1].to_string()
        })
    }
}

impl From<pb_messages::NodeId> for NodeId {
    fn from(mut pb_node_id: pb_messages::NodeId) -> NodeId {
        NodeId {
            name: pb_node_id.take_name(),
            addr: pb_node_id.take_addr()
        }
    }
}

impl From<NodeId> for pb_messages::NodeId {
    fn from(node_id: NodeId) -> pb_messages::NodeId {
        let mut pb_node_id = pb_messages::NodeId::new();
        pb_node_id.set_name(node_id.name);
        pb_node_id.set_addr(node_id.addr);
        pb_node_id
    }
}
