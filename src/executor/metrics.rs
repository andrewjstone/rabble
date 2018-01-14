#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutorMetrics {
    pub received_envelopes: u64,
}

impl ExecutorMetrics {
    pub fn new() -> ExecutorMetrics {
        ExecutorMetrics {
            received_envelopes: 0
        }
    }
}
