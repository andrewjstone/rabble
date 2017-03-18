use std::fmt::{Display, Error, Formatter};
use std::str::FromStr;
use std::convert::From;
use node_id::NodeId;
use pb_messages;

/// A globally unique process id
///
/// Pids can be grouped together for various reasons. This grouping acts like a namespace. If
/// a Process is not a member of a group, the `group` member of the Pid will be `None`.
//
// RustcEncodable and RustcDecodable derived so that MsgpackSerializer can use Pids
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, RustcEncodable, RustcDecodable)]
pub struct Pid {
    pub group: Option<String>,
    pub name: String,
    pub node: NodeId,
}

impl Display for Pid {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        match self.group {
            None => write!(f, "{}::{}", self.name, self.node),
            Some(ref g) => write!(f, "{}::{}::{}", g, self.name, self.node)
        }
    }
}

impl FromStr for Pid {
    type Err = String;

    fn from_str(s: &str) -> Result<Pid, String> {
        let v: Vec<&str> = s.split("::").collect();
        match v.len() {
            2 => Ok(Pid {
                group: None,
                name: v[0].to_string(),
                node: try!(NodeId::from_str(v[1]))
            }),
            3 => Ok(Pid {
                group: Some(v[0].to_string()),
                name: v[1].to_string(),
                node: try!(NodeId::from_str(v[2]))
            }),
            _ => return Err(
                "Invalid Pid format - Must be of form 'name::node' or \
                'group::name::node'".to_string()
            )
        }
    }
}
                
impl From<pb_messages::Pid> for Pid {
    fn from(mut pb_pid: pb_messages::Pid) -> Pid {
        let group = if pb_pid.has_group() {
            Some(pb_pid.take_group())
        } else {
            None
        };
        Pid {
            name: pb_pid.take_name(),
            group: group,
            node: pb_pid.take_node().into()
        }
    }
}

impl From<Pid> for pb_messages::Pid {
    fn from(pid: Pid) -> pb_messages::Pid {
        let mut pb_pid = pb_messages::Pid::new();
        pb_pid.set_name(pid.name);
        if let Some(group) = pid.group {
            pb_pid.set_group(group);
        }
        pb_pid.set_node(pid.node.into());
        pb_pid
    }
}
