use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use std::fmt::Debug;
use amy::{self, Poller, Registrar, Notification};
use pid::Pid;
use rustc_serialize::{Encodable, Decodable};
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use service_handler::ServiceHandler;
use envelope::{Envelope, SystemEnvelope};
use node::Node;
use errors::*;

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
    handler: H
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
        let msg = ExecutorMsg::RegisterSystemThread(pid.clone(), tx.clone());
        if let Err(_) = node.send(msg) {
            return Err(format!("Failed to send system thread registration from {}", pid).into());
        }
        try!(handler.init(&registrar, &node));
        Ok(Service {
            pid: pid,
            request_count: 0,
            tx: tx,
            rx: rx,
            node: node,
            poller: poller,
            registrar: registrar,
            handler: handler
        })
    }

    pub fn wait(&mut self) {
        loop {
            // TODO: Configurable timeout?
            for notification in self.poller.wait(1000).unwrap() {
                if notification.id == self.rx.get_id() {
                    if let Err(e) = self.handle_system_envelopes() {
                        if let ErrorKind::Shutdown(_) = *e.kind() {
                            // TODO: Log shutdown
                            println!("Service {}", e);
                            return;
                        }
                        println!("ERROR HANDLING SSYSTEM ENVELOPE {:?}", e);
                        // TODO: Log error
                    }
                } else {
                    if let Err(e) = self.handler.handle_notification(&self.node,
                                                                     notification,
                                                                     &self.registrar) {
                        //TODO: Log error
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
