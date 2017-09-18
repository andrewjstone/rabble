use std::fmt::Debug;
use amy::{self, Poller, Registrar};
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
    pub tx: amy::Sender<Envelope<T>>,
    rx: amy::Receiver<Envelope<T>>,
    node: Node<T>,
    poller: Poller,
    registrar: Registrar,
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
        let poller = Poller::new().unwrap();
        let mut registrar = poller.get_registrar()?;
        let (tx, rx) = registrar.channel()?;
        node.register_service(pid.clone(), tx.try_clone().unwrap())?;
        handler.init(&registrar, &node)?;
        let logger = node.logger.new(o!("component" => "service", "pid" => pid.to_string()));
        Ok(Service {
            pid: pid,
            tx: tx,
            rx: rx,
            node: node,
            poller: poller,
            registrar: registrar,
            handler: handler,
            logger: logger
        })
    }

    pub fn wait(&mut self) {
        loop {
            // TODO: Configurable timeout?
            for notification in self.poller.wait(1000).unwrap() {
                if notification.id == self.rx.get_id() {
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
                } else {
                    if let Err(e) = self.handler.handle_notification(&mut self.node,
                                                                     notification,
                                                                     &self.registrar) {
                        warn!(self.logger,
                               "Failed to handle poll notification";
                               "error" => e.to_string())
                    }
                }
            }
        }
    }

    pub fn handle_envelopes(&mut self) -> Result<()> {
        while let Ok(envelope) = self.rx.try_recv() {
            if let Msg::Shutdown = envelope.msg {
                return Err(ErrorKind::Shutdown(self.pid.clone()).into());
            }
            try!(self.handler.handle_envelope(&mut self.node, envelope, &self.registrar));
        }
        Ok(())
    }
}
