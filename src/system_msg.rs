use cluster_status::ClusterStatus;
use executor_status::ExecutorStatus;

pub enum SystemMsg<U> {
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    User(U)
}
