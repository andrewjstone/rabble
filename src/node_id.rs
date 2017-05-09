use std::fmt::{Display, Error, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
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
