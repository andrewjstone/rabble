use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use cluster_status::ClusterStatus;
use executor_status::ExecutorStatus;
use correlation_id::CorrelationId;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum Msg<T: Encodable + Decodable + Debug + Clone> {
    User(T),
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    Timeout,
    Shutdown
}
