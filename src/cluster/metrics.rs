#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub errors: u64,
    pub poll_notifications: u64,
    pub joins: u64,
    pub leaves: u64,
    pub received_local_envelopes: u64,
    pub received_remote_envelopes: u64,
    pub status_requests: u64,
    pub accepted_connections: u64,
    pub connection_attempts: u64
}

impl ClusterMetrics {
    pub fn new() -> ClusterMetrics {
        ClusterMetrics {
            errors: 0,
            poll_notifications: 0,
            joins: 0,
            leaves: 0,
            received_local_envelopes: 0,
            received_remote_envelopes: 0,
            status_requests: 0,
            accepted_connections: 0,
            connection_attempts: 0
        }
    }
}
