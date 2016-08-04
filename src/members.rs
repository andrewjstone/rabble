use std::collections::HashSet;
use std::fmt::{Display, Formatter, Error};
use orset::ORSet;
use node_id::NodeId;

#[derive(Debug, Clone)]
pub struct Members {
    pub me: NodeId,
    orset: ORSet<NodeId>
}

impl Display for Members {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let mut members = self.orset.elements();
        members.sort();
        for member in members {
            try!(fmt.write_fmt(format_args!("{} \n", member.name)));
        }
        Ok(())
    }
}

impl Members {
    pub fn new(node: NodeId) -> Members {
        let mut orset = ORSet::new(node.to_string());
        orset.add(node.clone());
        Members {
            me: node,
            orset: orset
        }
    }

    pub fn all(&self) -> HashSet<NodeId> {
        self.orset.elements().into_iter().collect()
    }

    pub fn join(&mut self, other: ORSet<NodeId>) {
        self.orset.join_state(other);
    }

    pub fn get_orset(&self) -> ORSet<NodeId> {
        self.orset.clone()
    }

    pub fn add(&mut self, element: NodeId) {
        self.orset.add(element);
    }
}
