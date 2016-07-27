use std::collections::HashSet;
use std::fmt::{Display, Formatter, Error};
use orset::ORSet;
use node::Node;

#[derive(Debug, Clone)]
pub struct Members {
    pub me: Node,
    orset: ORSet<Node>,
    connected: HashSet<Node>
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
    pub fn new(node: Node) -> Members {
        let mut orset = ORSet::new(node.to_string());
        orset.add(node.clone());
        Members {
            me: node,
            orset: orset,
            connected: HashSet::new()
        }
    }

    pub fn status(&self) -> MemberStatus {
        let all: HashSet<Node> = self.orset.elements().iter().filter(|&node| {
            *node != self.me
        }).cloned().collect();
        MemberStatus {
            connected: self.connected.clone(),
            disconnected: all.difference(&self.connected).cloned().collect()
        }
    }

    pub fn join(&mut self, other: ORSet<Node>) {
        self.orset.join_state(other);
    }

    pub fn get_orset(&self) -> ORSet<Node> {
        self.orset.clone()
    }

    pub fn add(&mut self, element: Node) {
        self.orset.add(element);
    }
}

#[derive(Debug, Clone)]
pub struct MemberStatus {
    connected: HashSet<Node>,
    disconnected: HashSet<Node>
}
