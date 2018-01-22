use histogram::Histogram;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutorMetrics {
    pub received_envelopes: u64,
    pub route_envelope_ns: Histogram

}

impl ExecutorMetrics {
    pub fn new() -> ExecutorMetrics {
        ExecutorMetrics {
            received_envelopes: 0,
            route_envelope_ns: Histogram::new()
        }
    }
}
