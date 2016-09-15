use pid::Pid;

pub struct ExecutorStatus {
    pub total_processes: usize,
    pub system_threads: Vec<Pid>,
    //... Some stats
}
