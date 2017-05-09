use pid::Pid;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutorStatus {
    pub total_processes: usize,
    pub services: Vec<Pid>,
    //... Some stats
}
