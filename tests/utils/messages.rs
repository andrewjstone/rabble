use rabble::Pid;

// Messages sent to processes
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ProcessMsg {
    Op(usize),
    GetHistory
}

// Messages sent to system threads
#[derive(Debug, Clone)]
pub enum SystemUserMsg {
    History(Vec<usize>),
    OpComplete
}

// Messages sent over the API server TCP connections
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ApiClientMsg {
    Op(Pid, usize),
    OpComplete,
    GetHistory(Pid),
    History(Vec<usize>)
}

