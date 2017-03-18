use std::fmt::Debug;
use errors::Error;

pub trait UserMsg: Debug + Clone + PartialEq + Send {
    fn to_bytes(self) -> Vec<u8>;
    fn from_bytes(Vec<u8>) -> Result<Self, Error>;
}
