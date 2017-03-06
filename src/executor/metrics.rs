use metrics::{Metric, Metrics};

metrics!(ExecutorMetrics {
    processes: i64,
    services: i64,
    received_envelopes: u64,
    timers_started: u64,
    timers_cancelled: u64
});
