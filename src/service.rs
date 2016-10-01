use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use std::fmt::Debug;
use std; // needed for slog
use amy::{self, Poller, Registrar, Notification};
use pid::Pid;
use rustc_serialize::{Encodable, Decodable};
use cluster_msg::ClusterMsg;
use service_handler::ServiceHandler;
use envelope::{Envelope, SystemEnvelope};
use node::Node;
use errors::*;
use slog;

/// A system service that operates on a single thread. A service is registered via its pid
/// with the executor and can send and receive messages to processes as well as other services.
pub struct Service<T, U, H>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          H: ServiceHandler<T, U>
{
    pub pid: Pid,
    request_count: usize,
    pub tx: amy::Sender<SystemEnvelope<U>>,
    rx: amy::Receiver<SystemEnvelope<U>>,
    node: Node<T, U>,
    poller: Poller,
    registrar: Registrar,
    handler: H,
    logger: slog::Logger
}

impl<T, U, H> Service<T, U, H>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          H: ServiceHandler<T, U>
{
    pub fn new(pid: Pid, node: Node<T, U>, mut handler: H) -> Result<Service<T, U, H>> {
        let poller = Poller::new().unwrap();
        let registrar = poller.get_registrar();
        let (tx, rx) = registrar.channel().unwrap();
        try!(node.register_system_thread(&pid, &tx));
        try!(handler.init(&registrar, &node));
        let logger = node.logger.new(o!("component" => "service"));
        Ok(Service {
            pid: pid,
            request_count: 0,
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
                    if let Err(e) = self.handle_system_envelopes() {
                        if let ErrorKind::Shutdown(_) = *e.kind() {
                            info!(self.logger, "Service shutting down";
                                  "pid" => self.pid.to_string());
                            return;
                        }
                        error!(self.logger,
                               "Failed to handle system envelope";
                               "error" => e.to_string())
                    }
                } else {
                    if let Err(e) = self.handler.handle_notification(&self.node,
                                                                     notification,
                                                                     &self.registrar) {
                        error!(self.logger,
                               "Failed to handle poll notification";
                               "error" => e.to_string())
                    }
                }
            }
        }
    }

    pub fn handle_system_envelopes(&mut self) -> Result<()> {
        while let Ok(envelope) = self.rx.try_recv() {
            if envelope.contains_shutdown_msg() {
                return Err(ErrorKind::Shutdown(self.pid.clone()).into());
            }
            try!(self.handler.handle_system_envelope(&self.node, envelope, &self.registrar));
        }
        Ok(())
    }
}
