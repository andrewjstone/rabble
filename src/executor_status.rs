use pid::Pid;

pub struct ExecutorStatus {
    pub correlation_id: usize,
    pub total_processes: usize,
    pub system_threads: Vec<Pid>,
    //... Some stats
}
