use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::fmt::{Display, Formatter, Error};
use orset::ORSet;
use node::Node;

#[derive(Debug, Clone)]
pub struct Members {
    pub me: Node,
    inner: Arc<RwLock<Inner>>
}

impl Display for Members {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let inner = self.inner.read().unwrap();
        let mut members = inner.orset.elements();
        members.sort();
        for member in members {
            try!(fmt.write_fmt(format_args!("{} \n", member.name)));
        }
        Ok(())
    }
}

impl Members {
    pub fn new(name: String, addr: String) -> Members {
        let me = Node {
            name: name.clone(),
            addr: addr,
        };
        let mut orset = ORSet::new(me.to_string());
        orset.add(me.clone());
        Members {
            me: me,
            inner: Arc::new(RwLock::new(Inner {
                orset: orset,
                connected: HashSet::new()
            }))
        }
    }

    pub fn status(&self) -> MemberStatus {
        let inner = self.inner.read().unwrap();
        let all: HashSet<Node> = inner.orset.elements().iter().filter(|&node| {
            *node != self.me
        }).cloned().collect();
        MemberStatus {
            connected: inner.connected.clone(),
            disconnected: all.difference(&inner.connected).cloned().collect()
        }
    }

    pub fn join(&mut self, other: ORSet<Node>) {
        let mut inner = self.inner.write().unwrap();
        inner.orset.join_state(other);
    }

    pub fn get_orset(&self) -> ORSet<Node> {
        let inner = self.inner.write().unwrap();
        inner.orset.clone()
    }

    pub fn add(&mut self, element: Node) {
        let mut inner = self.inner.write().unwrap();
        inner.orset.add(element);
    }
}

#[derive(Debug, Clone)]
struct Inner {
    orset: ORSet<Node>,
    connected: HashSet<Node>
}

#[derive(Debug, Clone)]
pub struct MemberStatus {
    connected: HashSet<Node>,
    disconnected: HashSet<Node>
}
