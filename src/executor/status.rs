use pid::Pid;
use super::ExecutorMetrics;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutorStatus {
    pub total_processes: usize,
    pub services: Vec<Pid>,
    pub metrics: ExecutorMetrics,
}
