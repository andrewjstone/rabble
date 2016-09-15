use cluster_status::ClusterStatus;
use executor_status::ExecutorStatus;

#[derive(Debug, Clone)]
pub enum SystemMsg<U> {
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    User(U)
}
