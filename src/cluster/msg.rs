use std::convert::{TryFrom, TryInto};
use amy::Notification;
use rustc_serialize::{Encodable, Decodable};
use msgpack::{Encoder, Decoder};
use errors::{Error, ChainErr};
use orset::{ORSet, Delta};
use node_id::NodeId;
use envelope::Envelope;
use user_msg::UserMsg;
use pb_messages;

/// Messages sent to the Cluster Server
pub enum ClusterMsg<T: UserMsg> {
    PollNotifications(Vec<Notification>),
    Join(NodeId),
    Leave(NodeId),
    Envelope(Envelope<T>),
    Shutdown
}

/// A message sent between nodes in Rabble.
///
#[derive(Debug, Clone)]
pub enum ExternalMsg<T: UserMsg> {
   Members {from: NodeId, orset: ORSet<NodeId>},
   Ping,
   Envelope(Envelope<T>),
   Delta(Delta<NodeId>)
}

impl<T: UserMsg> TryFrom<pb_messages::ClusterServerMsg> for ExternalMsg<T> {
    type Error = Error;
    fn try_from(mut pb_msg: pb_messages::ClusterServerMsg) -> Result<ExternalMsg<T>, Error> {
        if pb_msg.has_envelope() {
            return Ok(ExternalMsg::Envelope(pb_msg.take_envelope().try_into()?));
        }
        if pb_msg.has_ping() {
            return Ok(ExternalMsg::Ping);
        }
        if pb_msg.has_orset() {
            let mut pb_orset = pb_msg.take_orset();
            if !pb_orset.has_from() {
                return Err("Missing 'from' pid in ExternalMsg::Members".into());
            }
            let from = pb_orset.take_from().into();
            // ORsets are serialized as msgpack data still
            let serialized_orset = pb_orset.take_orset();
            let mut decoder = Decoder::new(&serialized_orset[..]);
            let orset = try!(Decodable::decode(&mut decoder)
                           .chain_err(|| format!("Failed to decode orset from {}", from)));
            return Ok(ExternalMsg::Members {from: from, orset: orset});
        }
        if pb_msg.has_delta() {
            let serialized = pb_msg.take_delta();
            let mut decoder = Decoder::new(&serialized[..]);
            let delta = try!(Decodable::decode(&mut decoder)
                             .chain_err(|| "Failed to decode ExternalMsg::Delta"));
            return Ok(ExternalMsg::Delta(delta));
        }
        Err("Received unkown ExternalMsg".into())
    }
}

impl<T: UserMsg> TryFrom<ExternalMsg<T>> for pb_messages::ClusterServerMsg {
    type Error = Error;
    fn try_from(msg: ExternalMsg<T>) -> Result<pb_messages::ClusterServerMsg, Error> {
        let mut pb_msg = pb_messages::ClusterServerMsg::new();
        match msg {
            ExternalMsg::Envelope(envelope) => {
                pb_msg.set_envelope(envelope.into());
            },
            ExternalMsg::Ping => {
                pb_msg.set_ping(true);
            },
            ExternalMsg::Members {from, orset} => {
                let mut pb_orset = pb_messages::MemberORSet::new();
                pb_orset.set_from(from.clone().into());
                let mut encoded_orset = Vec::new();
                orset.encode(&mut Encoder::new(&mut encoded_orset))
                    .chain_err(|| format!("Failed to encode orset from {}", from))?;
                pb_orset.set_orset(encoded_orset);
                pb_msg.set_orset(pb_orset);
            },
            ExternalMsg::Delta(delta) => {
                let mut encoded = Vec::new();
                delta.encode(&mut Encoder::new(&mut encoded))
                    .chain_err(|| format!("Failed to encode delta"))?;
                pb_msg.set_delta(encoded);
            }
        }
        Ok(pb_msg)
    }
}
