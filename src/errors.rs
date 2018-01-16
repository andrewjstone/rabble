use std::{self, io};
use msgpack;
use protobuf;
use pid::Pid;
use node_id::NodeId;

/// Used by the error-chain crate to generate errors
error_chain! {
    foreign_links {
        io::Error, Io;
        std::sync::mpsc::RecvError, RecvError;
        msgpack::encode::Error, MsgpackEncode;
        msgpack::decode::Error, MsgpackDecode;
        protobuf::error::ProtobufError, Protobuf;
    }

    errors {
        EncodeError(id: Option<usize>, to: Option<NodeId>) {
            description("Failed to encode message")
            display("Failed to encode message to {:?}, id={:?}", to, id)
        }
        DecodeError(id: usize, from: Option<NodeId>) {
            description("Failed to decode message")
            display("Failed to decode message from {:?}, id={}", from, id)
        }
        RegistrarError(id: Option<usize>, node: Option<NodeId>) {
            description("Failed to register/deregister/reregister socket")
            display("Failed to register/deregister/reregister socket: id={:?}, peer={:?}", id, node)
        }
        WriteError(id: usize, node: Option<NodeId>) {
            description("Failed to write to socket")
            display("Failed to write to socket: id={}, peer={:?}", id, node)
        }
        ReadError(id: usize, node: Option<NodeId>) {
            description("Failed to read from socket")
            display("Failed to read from socket: id={}, peer={:?}", id, node)
        }
        BroadcastError(errors: Vec<Error>) {
            description("Failed to broadcast")
            display("Failed to broadcast: errors = {:?}", errors)
        }
        PollNotificationErrors(errors: Vec<Error>) {
            description("Failed to process poll notifications")
            display("Failed to process poll notifications: errors = {:?}", errors)
        }
        ConnectError(node: NodeId) {
            description("Failed to connect")
            display("Failed to connect to {}", node)
        }
        SendError(msg: String, pid: Option<Pid>) {
            description("Failed to send")
            display("Failed to send {} to {:?}", msg, pid)
        }
        Shutdown(pid: Pid) {
            description("Shutting down")
            display("Shutting down {}", pid)
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
