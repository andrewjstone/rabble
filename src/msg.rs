use cluster::ClusterStatus;
use executor::ExecutorStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Msg<T> {
    User(T),
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    Timeout,
    Shutdown
}
