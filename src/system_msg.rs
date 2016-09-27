use cluster_status::ClusterStatus;
use executor_status::ExecutorStatus;
use correlation_id::CorrelationId;

#[derive(Debug, Clone)]
pub enum SystemMsg<U> {
    ClusterStatus(ClusterStatus),
    ExecutorStatus(ExecutorStatus),
    Timeout,
    User(U),
    Shutdown
}
