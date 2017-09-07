use cluster::ClusterStatus;
use executor::ExecutorStatus;
use correlation_id::CorrelationId;
use metrics::Metric;
use msg_registry::Registry;

/// MsgIds for Rabble specific messages start at 2^31
const RabbleMsgOffset: u32 = 2^31;

type Name = String;

/// Requests made to rabble that may or may not expect replies
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    GetMetrics
}

/// Replies from rabble in response to requests
#[derive(Debug, Serialize, Deserialize)]
pub enum Reply {
    Cluster(ClusterStatus),
    Executor(ExecutorStatus),
    Metrics(Vec<(Name, Metric)>)
}

/// Asynchronous notifications from Rabble
#[derive(Debug, Serialize, Deserialize)]
pub enum Notify {
    Timeout,
    Shutdown
}

/// Register all rabble specific messages
pub fn register(registry: Registry) {
    register!(registry, {
        Request => RabbleMsgOffset,
        Reply => RabbleMsgOffset + 1,
        Notify => RabbleMsgOffset + 2
    });
}
