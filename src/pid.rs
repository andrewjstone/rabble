use std::fmt::{Display, Error, Formatter};
use std::str::FromStr;
use node_id::NodeId;

/// A globally unique process id
///
/// Pids can be grouped together for various reasons. This grouping acts like a namespace. If
/// a Process is not a member of a group, the `group` member of the Pid will be `None`.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
