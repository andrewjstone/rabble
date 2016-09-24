use std::fmt::Debug;
use std::marker::Send;
use rustc_serialize::{Encodable, Decodable};
use amy::{Notification, Registrar};
use envelope::SystemEnvelope;
use node::Node;
use errors::*;

/// A service handler
pub trait ServiceHandler<T: Encodable + Decodable, U: Debug + Clone> {
    /// A callback function used to initialize the handler.
    ///
    /// The handler is expected to register any necessary timeouts or listening sockets with the
    /// poller and send any initialization messages via the Node. Some handlers may not need any
    /// initialization, so this callback is optional.
    fn init(&mut self, &Registrar, &Node<T, U>) -> Result<()> {
        Ok(())
    }

    /// Handle poll notifications.

    /// Some handler don't register anything that requires notification and only receive system
    /// envelopes. Those handlers do not need to implement this function.
    fn handle_notification(&mut self, &Node<T, U>, Notification, &Registrar) -> Result<()> {
        // TODO: Log message
        Ok(())
    }

    /// Handle any system envelopes addressed to the Service's Pid. All handlers must implement
    /// this function.
    fn handle_system_envelope(&mut self, &Node<T, U>, SystemEnvelope<U>, &Registrar) -> Result<()>;
}
