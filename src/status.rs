use std::collections::{HashMap, HashSet};
use histogram::Histogram;

pub type StatusTable = HashMap<String, StatusVal>;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds
}

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum StatusVal {
    Histogram(TimeUnit, Histogram),
    Int(u64),
    String(String),
    StringSet(HashSet<String>),
    Timestamp(String) // UTC
}

impl StatusVal {
    pub fn get_histogram(self) -> (TimeUnit, Histogram) {
        if let StatusVal::Histogram(unit, histogram) = self {
            return (unit, histogram);
        }
        unreachable!();
    }

    pub fn get_int(self) -> u64 {
        if let StatusVal::Int(n) = self {
            return n;
        }
        unreachable!();
    }

    pub fn get_string(self) -> String {
        if let StatusVal::String(s) = self {
            return s;
        }
        unreachable!();
    }

    pub fn get_stringset(self) -> HashSet<String> {
        if let StatusVal::StringSet(s) = self {
            return s;
        }
        unreachable!();
    }

    pub fn get_timestamp(self) -> String {
        if let StatusVal::Timestamp(s) = self {
            return s;
        }
        unreachable!();
    }
}
