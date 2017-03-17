use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use cluster::ClusterStatus;
use executor::ExecutorStatus;
use correlation_id::CorrelationId;
use metrics::Metric;
use pid::Pid;

type Name = String;

#[derive(Debug, Clone, PartialEq, RustcEncodable, RustcDecodable)]
pub enum Msg<T: Encodable + Decodable + Debug + Clone> {
    User(T),
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    StartTimer(usize), // time in ms
    CancelTimer(Option<CorrelationId>),
    Timeout,
    Shutdown,
    GetMetrics,
    Metrics(Vec<(Name, Metric)>),
    Pids(Vec<Pid>)
}
