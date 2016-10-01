use std::io;
use msgpack;
use pid::Pid;
use node_id::NodeId;

/// Used by the error-chain crate to generate errors
error_chain! {
    foreign_links {
        io::Error, Io;
        msgpack::encode::Error, MsgpackEncode;
        msgpack::decode::Error, MsgpackDecode;
    }

    errors {
        EncodeError(id: Option<usize>, to: Option<NodeId>) {
            description("failed to encode message")
            display("failed to encode message to {:?}, id={:?}", to, id)
        }
        DecodeError(id: usize, from: Option<NodeId>) {
            description("failed to decode message")
            display("failed to decode message from {:?}, id={}", from, id)
        }
        RegistrarError(id: Option<usize>, node: Option<NodeId>) {
            description("failed to register/deregister/reregister socket")
            display("failed to register/deregister/reregister socket: id={:?}, peer={:?}", id, node)
        }
        WriteError(id: usize, node: Option<NodeId>) {
            description("failed to write to socket")
            display("failed to write to socket: id={}, peer={:?}", id, node)
        }
        ReadError(id: usize, node: Option<NodeId>) {
            description("failed to read from socket")
            display("failed to read from socket: id={}, peer={:?}", id, node)
        }
        BroadcastError(errors: Vec<Error>) {
            description("failed to broadcast")
            display("failed to broadcast: errors = {:?}", errors)
        }
        PollNotificationErrors(errors: Vec<Error>) {
            description("failed to process poll notifications")
            display("failed to process poll notifications: errors = {:?}", errors)
        }
        ConnectError(node: NodeId) {
            description("failed to connect")
            display("failed to connect to {}", node)
        }
        SendError(msg: String, pid: Option<Pid>) {
            description("failed to send")
            display("failed to send {} to {:?}", msg, pid)
        }
        Shutdown(pid: Pid) {
            description("shutting down")
            display("shutting down {}", pid)
        }
    }
}

impl ErrorKind {
    /// Return the socket ids of the error if there are any
    pub fn get_ids(&self) -> Vec<usize> {
        match *self {
            ErrorKind::EncodeError(id, _) => id.map_or(vec![], |id| vec![id]),
            ErrorKind::DecodeError(id, _) => vec![id],
            ErrorKind::RegistrarError(id, _) => id.map_or(vec![], |id| vec![id]),
            ErrorKind::WriteError(id, _) => vec![id],
            ErrorKind::ReadError(id, _) => vec![id],
            ErrorKind::BroadcastError(ref errors) =>
                errors.iter().flat_map(|e| e.kind().get_ids()).collect(),
            ErrorKind::PollNotificationErrors(ref errors) =>
                errors.iter().flat_map(|e| e.kind().get_ids()).collect(),

            _ => vec![]
        }
    }
}
