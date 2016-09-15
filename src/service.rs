use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::HashMap;
use amy::{Poller, Registrar};
use pid::Pid;
use rustc_serialize::{Encodable, Decodable};
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use handler::Handler;
use envelope::{Envelope, SystemEnvelope};

type HandlerId = usize;

/// A system service that operates on a single thread. A service is registered via its pid
/// with the executor and can send and receive messages to processes as well as other services.
pub struct Service<T: Encodable + Decodable, U> {
    pid: Pid,
    request_count: usize,
    tx: Sender<SystemEnvelope<U>>,
    rx: Receiver<SystemEnvelope<U>>,
    executor_tx: Sender<ExecutorMsg<T, U>>,
    cluster_tx: Sender<ClusterMsg<T>>,
    poller: Option<Poller>,
    handlers: Vec<Box<Handler<T, U>>>
}

impl<T: Encodable + Decodable, U> Service<T, U> {
    pub fn new(pid: Pid,
               executor_tx: Sender<ExecutorMsg<T, U>>,
               cluster_tx: Sender<ClusterMsg<T>>) -> Service<T, U> {
        let (tx, rx) = channel();
        let msg = ExecutorMsg::RegisterSystemThread(pid.clone(), tx.clone());
        executor_tx.send(msg).unwrap();
        Service {
            pid: pid,
            request_count: 0,
            tx: tx,
            rx: rx,
            executor_tx: executor_tx,
            cluster_tx: cluster_tx,
            poller: None,
            handlers: Vec::new()
        }
    }

    /// Add a new handler and return it's Id
    pub fn add_handler(&mut self, mut handler: Box<Handler<T, U>>) -> usize {
        let handler_id = self.handlers.len();
        handler.register(self, handler_id);
        self.handlers.push(handler);
        handler_id
    }

    /// Called by the Handler::register() to create a poller if needed by the handler
    ///
    /// Only one poller is ever created. If a poller already exists it is shared by handlers that
    /// require a poller.
    pub fn create_poller(&mut self) {
        if self.poller.is_none() {
            self.poller = Some(Poller::new().unwrap());
        }
    }
}
