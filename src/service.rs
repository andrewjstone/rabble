use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use std::fmt::Debug;
use amy::{self, Poller, Registrar};
use pid::Pid;
use rustc_serialize::{Encodable, Decodable};
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use handler::Handler;
use envelope::{Envelope, SystemEnvelope};
use node::Node;

type PollId = usize;
type HandlerId = usize;

pub enum ServiceError {
    DefaultHandlerAlreadyExists
}

/// A system service that operates on a single thread. A service is registered via its pid
/// with the executor and can send and receive messages to processes as well as other services.
pub struct Service<T: Encodable + Decodable, U: Debug + Clone> {
    pid: Pid,
    request_count: usize,
    rx: amy::Receiver<SystemEnvelope<U>>,
    node: Node<T, U>,
    poller: Poller,
    registrar: Registrar,
    poll_ids: HashMap<PollId, HandlerId>,
    default_handler_id: Option<HandlerId>,
    handlers: Vec<Box<Handler<T, U>>>
}

impl<T: Encodable + Decodable, U: Debug + Clone> Service<T, U> {
    pub fn new(name: &str, node: Node<T, U>) -> Service<T, U> {
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
            poll_ids: HashMap::new(),
            default_handler_id: None,
            handlers: Vec::new()
        }
    }

    /// Add a new handler and return it's Id. Return an error if this handler attempts to set a
    /// default handler when one already exists.
    pub fn add_handler(&mut self, mut handler: Box<Handler<T, U>>) -> Result<usize, ServiceError> {
        let handler_id = self.handlers.len();
        handler.set_id(handler_id);
        let spec = handler.get_spec();

        // The Default handler handle any SystemEnvelopes without a correlation id
        if spec.default_handler {
            if self.default_handler_id.is_some() {
                return Err(ServiceError::DefaultHandlerAlreadyExists)
            }
            self.default_handler_id = Some(handler_id);
        }

        if spec.requires_poller {
            // Keep track of which handler corresponds to each poll id
            for poll_id in handler.register_with_poller(&self.registrar) {
                self.poll_ids.insert(poll_id, handler_id);
            }
        }

        self.handlers.push(handler);
        Ok(handler_id)
    }

    pub fn wait(&mut self) {
        // TODO: Configurable timeout?
        let notifications = self.poller.wait(1000).unwrap();
    }
}
