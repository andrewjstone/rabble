use std::fmt::Debug;
use std::sync::mpsc::{channel, Sender, Receiver};
use pid::Pid;
use serde::{Serialize, Deserialize};
use msg::Msg;
use envelope::Envelope;
use node::Node;
use errors::*;
use slog;
use super::ServiceHandler;

/// A system service that operates on a single thread. A service is registered via its pid
/// with the executor and can send and receive messages to processes as well as other services.
pub struct Service<T, H> {
    pub pid: Pid,
    pub tx: Sender<Envelope<T>>,
    rx: Receiver<Envelope<T>>,
    node: Node<T>,
    handler: H,
    logger: slog::Logger
}

impl<'de, T, H> Service<T, H>
    where T: Serialize + Deserialize<'de> + Debug + Clone,
          H: ServiceHandler<T>
{
    pub fn new(pid: Pid, node: Node<T>, mut handler: H)
        -> Result<Service<T, H>>
    {
        let (tx, rx) = channel();
        node.register_service(&pid, &tx)?;
        handler.init(&node)?;
        let logger = node.logger.new(o!("component" => "service", "pid" => pid.to_string()));
        Ok(Service {
            pid: pid,
            tx: tx,
            rx: rx,
            node: node,
            handler: handler,
            logger: logger
        })
    }

    pub fn wait(&mut self) {
        if let Err(e) = self.handle_envelopes() {
            if let ErrorKind::Shutdown(_) = *e.kind() {
                info!(self.logger, "Service shutting down";
                      "pid" => self.pid.to_string());
                return;
            }
            error!(self.logger,
                   "Failed to handle envelope";
                   "error" => e.to_string())
        }
    }

    pub fn handle_envelopes(&mut self) -> Result<()> {
        loop {
            let envelope = self.rx.recv()?;
            if let Msg::Shutdown = envelope.msg {
                return Err(ErrorKind::Shutdown(self.pid.clone()).into());
            }
            self.handler.handle_envelope(&self.node, envelope)?;
        }
    }
}
