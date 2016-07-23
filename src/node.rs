use std::fmt::{Display, Error, Formatter};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, RustcEncodable, RustcDecodable)]
pub struct Node {
    pub name: String,
    pub addr: String
}

impl Display for Node {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        try!(fmt.write_fmt(format_args!("{}::{}", self.name, self.addr)));
        Ok(())
    }
}
