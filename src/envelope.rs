use std::fmt::Debug;
use serde::Serialize;
use bincode::{serialize, Bounded, Error};
use pid::Pid;
use correlation_id::CorrelationId;

/// Max msg size = 1MB
const MaxMsgSize: Bounded = Bounded(1024*1024);

/// All Msgs are dynamically typed.
///
/// Each Msg has a globally unique id throughout the cluster
#[derive(Debug, Serialize, Deserialize)]
pub struct Msg {
    pub id: MsgId,
    pub ty: MsgType
}

/// All messages are dynamically typed, and have a different representation when packaged into
/// envelopes.
///
/// Messages destined for local processes are encoded as Box<Any>, while those destined for
/// processes on remote nodes are serialized into a Vec<u8>.
#[derive(Debug, Serialize, Deserialize)]
pub enum MsgType {
    // We can only serialize and deserialize remote variants
    #[serde(skip_serializing, skip_deserializing)]
    Local(Box<Any>),
    Remote(Vec<u8>)
}

/// Envelopes are routable to processes on all nodes and threads running on the same node as this
/// process.
#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub to: Pid,
    pub from: Pid,
    pub msg: Msg,
    pub correlation_id: Option<CorrelationId>
}

impl Envelope {

    /// Create a new envelope.
    ///
    /// Return an error if the destination is remote and the msg cannot be serialized.
    ///
    /// Panic if the destination is remote, but the TypeId does not exist in the registry.
    pub fn new<T>(to: Pid,
                  from: Pid,
                  msg: T,
                  cid: Option<CorrelationId>) -> Result<Envelope, bincode::Error>
        where T: Serialize + 'static
    {
        // Panic on purpose if the TypeId does not exist in the registry..
        // We want to know immediately if we are trying to serialize an unknown type.
        let id = MSG_REGISTRY.get_msg_id(TypeId::of::<T>()).unwrap();
        let msg_type = if to.node == from.node {
            MsgType::Local(Box::new(msg) as Box<Any>)
        } else {
            let buf = serialize(&msg, MaxMsgSize)?;
            MsgType::Remote(buf)
        };

        let msg = Msg {
            id: id,
            ty: msg_type
        };

        Ok(Envelope {
            to: to,
            from: from,
            msg: msg,
            correlation_id: cid
        })
    }
}
