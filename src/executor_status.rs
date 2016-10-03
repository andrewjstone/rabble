use pid::Pid;

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct ExecutorStatus {
    pub total_processes: usize,
    pub system_threads: Vec<Pid>,
    //... Some stats
}
