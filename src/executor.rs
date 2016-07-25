use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node::Node;

pub struct Executor<T, P> where P: Process<T> {
    node: Node,
    processes: HashMap<Pid, P>,
    tx: Sender<Envelope<T>>,
    rx: Receiver<Envelope<T>>,
    cluster_tx: Sender<Envelope<T>>
}

impl<T, P> Executor<T, P> where P: Process<T> {
    pub fn new(node: Node,
               tx: Sender<Envelope<T>>,
               rx: Receiver<Envelope<T>>,
               cluster_tx: Sender<Envelope<T>>) -> Executor<T, P> {
        Executor {
            node: node,
            processes: HashMap::new(),
            tx: tx,
            rx: rx,
            cluster_tx: cluster_tx
        }
    }

    /// Run the executor
    ///
    ///This call blocks the current thread indefinitely.
    pub fn run(mut self) {
        while let Ok(Envelope {to, from, msg}) = self.rx.recv() {
            if let Some(process) = self.processes.get_mut(&to) {
                for envelope in process.handle(msg, from).drain(..) {
                    if envelope.to.node == self.node {
                        // This won't ever fail because we hold a ref to both ends of the channel
                        self.tx.send(envelope).unwrap();
                    } else {
                        // Return if the cluster server thread has exited
                        // The system is shutting down.
                        if let Err(_) = self.cluster_tx.send(envelope) {
                            return;
                        }
                    }
                }
            }
        }
    }
}
