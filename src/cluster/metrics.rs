use metrics::{Metric, Metrics};

metrics!(ClusterMetrics {
    errors: u64,
    poll_notifications: u64,
    joins: u64,
    leaves: u64,
    received_local_envelopes: u64,
    received_remote_envelopes: u64,
    status_requests: u64,
    accepted_connections: u64,
    connection_attempts: u64
});
