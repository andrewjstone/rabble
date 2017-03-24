use std::convert::{From, TryFrom, TryInto};
use errors::Error;
use pid::Pid;
use correlation_id::CorrelationId;
use msg::Msg;
use pb_messages;
use user_msg::UserMsg;

/// Envelopes are the the message type received by actors
///
/// Envelopes are routable to processes and services on all nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope<T: UserMsg> {
    pub to: Pid,
    pub from: Pid,
    pub msg: Msg<T>,
    pub correlation_id: CorrelationId
}

impl<T: UserMsg> TryFrom<pb_messages::Envelope> for Envelope<T> {
    type Error = Error;
    fn try_from(mut pb_envelope: pb_messages::Envelope) -> Result<Envelope<T>, Error> {
        Ok(Envelope {
            to: pb_envelope.take_to().into(),
            from: pb_envelope.take_from().into(),
            msg: pb_envelope.take_msg().try_into()?,
            correlation_id: pb_envelope.take_cid().into()
        })
    }
}

impl<T: UserMsg> From<Envelope<T>> for pb_messages::Envelope {
    fn from(envelope: Envelope<T>) -> pb_messages::Envelope {
        let mut pb_envelope = pb_messages::Envelope::new();
        pb_envelope.set_to(envelope.to.into());
        pb_envelope.set_from(envelope.from.into());
        pb_envelope.set_msg(envelope.msg.into());
        pb_envelope.set_cid(envelope.correlation_id.into());
        pb_envelope
    }
}
