use pid::Pid;

#[derive(Clone, Debug)]
pub struct ExecutorStatus {
    pub total_processes: usize,
    pub system_threads: Vec<Pid>,
    //... Some stats
}
