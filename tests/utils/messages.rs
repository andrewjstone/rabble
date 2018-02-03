use rabble::Pid;

// Msg type parameter for messages sent to processes and services
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TestMsg {
    Op(usize), // Request
    ForwardOp(usize, Pid), // Request forwarded with client pid included
    OpComplete, // Reply

    GetHistory, // Request
    History(Vec<usize>) // Reply
}
