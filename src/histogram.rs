use std::fmt::{self, Debug, Formatter};
use hdrsample;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds
}

#[derive(Clone, PartialEq, RustcEncodable, RustcDecodable)]
pub struct Histogram {
    pub unit: TimeUnit,
    pub histogram: hdrsample::Histogram<u64>
}

impl Debug for Histogram {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Histogram ({:?})", self.unit)
    }
}
