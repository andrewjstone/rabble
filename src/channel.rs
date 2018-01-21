use errors::{Result, ErrorKind};
use std::sync::mpsc;
use futures::sync::mpsc::UnboundedSender;
use amy;

/// A generic sender used to interact with code running outside the actor system.
///
/// This type is often used as a reply channel for request originating outside the actor system.
///
/// In order to prevent the actor system from blocking, senders must be unbounded. It is recommended
/// to add backpressure or ratelimiting at the boundaries of the system, rather than internal to the
/// actors, since actors do not generate messgaes on their own.
pub trait Sender<T: Send> : Send {
    fn send(&self, msg: T) -> Result<()>;
}

impl<T: Send> Sender<T> for UnboundedSender<T> {
    fn send(&self, msg: T) -> Result<()> {
        self.unbounded_send(msg).map_err(|e| {
            let msg = format!("Failed to send to future via mpsc: {}", e.to_string());
            ErrorKind::SendError(msg, None).into()
        })
    }
}

impl<T: Send> Sender<T> for mpsc::Sender<T> {
    fn send(&self, msg: T) -> Result<()> {
        self.send(msg).map_err(|e| {
            let msg = format!("Failed to send to mpsc receiver: {}", e.to_string());
            ErrorKind::SendError(msg, None).into()
        })
    }
}

impl<T: Send> Sender<T> for amy::Sender<T> {
    fn send(&self, msg: T) -> Result<()> {
        self.send(msg).map_err(|_| {
            let msg = format!("Failed to send to amy receiver");
            ErrorKind::SendError(msg, None).into()
        })
    }
}
