use amy::{Notification, Registrar};
use envelope::Envelope;
use node::Node;
use errors::*;

/// A service handler
pub trait ServiceHandler<T> {
    /// A callback function used to initialize the handler.
    ///
    /// The handler is expected to register any necessary timeouts or listening sockets with the
    /// poller and send any initialization messages via the Node. Some handlers may not need any
    /// initialization, so this callback is optional.
    fn init(&mut self, &Registrar, &Node<T>) -> Result<()> {
        Ok(())
    }

    /// Handle poll notifications.

    /// Some handler don't register anything that requires notification and only receive
    /// envelopes. Those handlers do not need to implement this function.
    fn handle_notification(&mut self, &Node<T>, Notification, &Registrar) -> Result<()> {
        // TODO: Log message
        Ok(())
    }

    /// Handle any envelopes addressed to the service's Pid. All handlers must implement
    /// this function.
    fn handle_envelope(&mut self, &Node<T>, Envelope<T>, &Registrar) -> Result<()>;
}
