
// Msg type parameter for messages sent to processes and services
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum RabbleUserMsg {
    Op(usize), // Request
    OpComplete, // Reply

    GetHistory, // Request
    History(Vec<usize>) // Reply
}
