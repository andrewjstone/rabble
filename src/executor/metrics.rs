use metrics::{Metric, Metrics};
use histogram::Histogram;

metrics!(ExecutorMetrics {
    processes: i64,
    services: i64,
    received_envelopes: u64,
    timers_started: u64,
    timers_cancelled: u64,
    route_envelope_ns: Histogram
});
