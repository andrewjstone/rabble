use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use std::fmt::Debug;
use amy::{self, Poller, Registrar, Notification};
use pid::Pid;
use rustc_serialize::{Encodable, Decodable};
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use handler::Handler;
use envelope::{Envelope, SystemEnvelope};
use node::Node;
use errors::*;

/// A system service that operates on a single thread. A service is registered via its pid
/// with the executor and can send and receive messages to processes as well as other services.
pub struct Service<T, U, H>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          H: Handler<T, U>
{
    pub pid: Pid,
    request_count: usize,
    rx: amy::Receiver<SystemEnvelope<U>>,
    node: Node<T, U>,
    poller: Poller,
    registrar: Registrar,
    handler: H
}

impl<T, U, H> Service<T, U, H>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          H: Handler<T, U>
{
    pub fn new(name: &str, node: Node<T, U>, handler: H) -> Service<T, U, H> {
        let pid = Pid {
            name: name.to_string(),
            group: Some("Service".to_string()),
            node: node.id.clone()
        };
        let poller = Poller::new().unwrap();
        let registrar = poller.get_registrar();
        let (tx, rx) = registrar.channel().unwrap();
        let msg = ExecutorMsg::RegisterSystemThread(pid.clone(), tx);
        node.send(msg).unwrap();
        Service {
            pid: pid,
            request_count: 0,
            rx: rx,
            node: node,
            poller: poller,
            registrar: registrar,
            handler: handler
        }
    }

    pub fn wait(&mut self) {
        // TODO: Configurable timeout?
        for notification in self.poller.wait(1000).unwrap() {
            if notification.id == self.rx.get_id() {
                if let Err(e) = self.handle_system_envelopes() {
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

    pub fn handle_system_envelopes(&mut self) -> Result<()> {
        while let Ok(envelope) = self.rx.try_recv() {
            try!(self.handler.handle_system_envelope(&self.node, envelope));
        }
        Ok(())
    }
}
