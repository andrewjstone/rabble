use std::collections::{HashMap, HashSet};
use histogram::Histogram;

pub type StatusTable = HashMap<String, StatusVal>;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum StatusVal {
    Ok,
    Warning,
    Error,
    Histogram(Histogram),
    Int(u64),
    String(String),
    StringSet(HashSet<String>),
    Timestamp(i64, i32) // Seconds, Nanoseconds
}

impl StatusVal {
    pub fn get_int(self) -> u64 {
        if let StatusVal::Int(n) = self {
            return n;
        }
        unreachable!();
    }

    pub fn get_stringset(self) -> HashSet<String> {
        if let StatusVal::StringSet(s) = self {
            return s;
        }
        unreachable!();
    }
}
