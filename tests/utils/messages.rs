use rabble::{Pid, UserMsg, Error};
use super::pb_rabble_user_msg::{self, PbRabbleUserMsg};
use protobuf::{Message, parse_from_bytes};

// Msg type parameter for messages sent to processes and services
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RabbleUserMsg {
    Op(u64), // Request
    OpComplete, // Reply

    GetHistory, // Request
    History(Vec<u64>) // Reply
}

// Messages sent over the API server TCP connections
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ApiClientMsg {
    Op(Pid, u64),
    OpComplete,
    GetHistory(Pid),
    History(Vec<u64>),
    Timeout
}

impl UserMsg for RabbleUserMsg {
    fn to_bytes(self) -> Vec<u8> {
        use RabbleUserMsg::*;
        let mut msg = PbRabbleUserMsg::new();
        match self {
            Op(num) => msg.set_op(num),
            OpComplete => msg.set_op_complete(true),
            GetHistory => msg.set_get_history(true),
            History(vec) => {
                let mut history = pb_rabble_user_msg::History::new();
                history.set_history(vec);
                msg.set_history(history);
            }
        }
        msg.write_to_bytes().unwrap()
    }

    fn from_bytes(bytes: Vec<u8>) -> Result<RabbleUserMsg, Error> {
        let mut msg: PbRabbleUserMsg = parse_from_bytes(&bytes[..])?;
        use RabbleUserMsg::*;
        if msg.has_op() {
            return Ok(Op(msg.get_op()));
        }
        if msg.has_op_complete() {
            return Ok(OpComplete);
        }
        if msg.has_get_history() {
            return Ok(GetHistory);
        }
        if msg.has_history() {
            return Ok(History(msg.take_history().take_history()));
        }
        Err("Invalid PbRabbleUserMsg".into())
    }
}
