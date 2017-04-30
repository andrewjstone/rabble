use rabble::Pid;

// Msg type parameter for messages sent to processes and services
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum RabbleUserMsg {
    Op(usize), // Request
    OpComplete, // Reply

    GetHistory, // Request
    History(Vec<usize>) // Reply
}

// Messages sent over the API server TCP connections
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ApiClientMsg {
    Op(Pid, usize),
    OpComplete,
    GetHistory(Pid),
    History(Vec<usize>),
    Timeout
}

