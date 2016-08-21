use rustc_serialize::{Encodable, Decodable};
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;

pub struct Executor<T: Encodable + Decodable + Send>{
    node: NodeId,
    processes: HashMap<Pid, Box<Process<T>>>,
    tx: Sender<ExecutorMsg<T>>,
    rx: Receiver<ExecutorMsg<T>>,
    cluster_tx: Sender<ClusterMsg<T>>
}

impl<T: Encodable + Decodable + Send> Executor<T> {
    pub fn new(node: NodeId,
               tx: Sender<ExecutorMsg<T>>,
               rx: Receiver<ExecutorMsg<T>>,
               cluster_tx: Sender<ClusterMsg<T>>) -> Executor<T> {
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
        while let Ok(msg) = self.rx.recv() {
            match msg {
                ExecutorMsg::User(envelope) => self.route(envelope),
                ExecutorMsg::Start(pid, process) => self.start(pid, process),
                ExecutorMsg::Stop(pid) => self.stop(pid)
            }
        }
    }

    fn start(&mut self, pid: Pid, process: Box<Process<T>>) {
        self.processes.insert(pid, process);
    }

    fn stop(&mut self, pid: Pid) {
        self.processes.remove(&pid);
    }

    /// Route envelopes to local or remote processes
    ///
    /// Retrieve any envelopes from processes handling local messages and put them on either the
    /// executor or the cluster channel depending upon whether they are local or remote.
    fn route(&mut self, envelope: Envelope<T>) {
        let Envelope {to, from, msg} = envelope;
        if let Some(process) = self.processes.get_mut(&to) {
            for envelope in process.handle(msg, from).drain(..) {
                if envelope.to.node == self.node {
                    // This won't ever fail because we hold a ref to both ends of the channel
                    self.tx.send(ExecutorMsg::User(envelope)).unwrap();
                } else {
                    // Return if the cluster server thread has exited
                    // The system is shutting down.
                    if let Err(_) = self.cluster_tx.send(ClusterMsg::User(envelope)) {
                        return;
                    }
                }
            }
        }
    }
}
