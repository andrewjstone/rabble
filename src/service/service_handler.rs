use envelope::Envelope;
use node::Node;
use errors::*;

/// A service handler
pub trait ServiceHandler<T> {
    /// A callback function used to initialize the handler.
    ///
    /// The handler is expected to send any initialization messages via the Node.
    /// Some handlers may not need any initialization, so this callback is optional.
    fn init(&mut self, &Node<T>) -> Result<()> {
        Ok(())
    }

    /// Handle any envelopes addressed to the service's Pid. All handlers must implement
    /// this function.
    fn handle_envelope(&mut self, &Node<T>, Envelope<T>) -> Result<()>;
}
