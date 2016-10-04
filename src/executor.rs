use rustc_serialize::{Encodable, Decodable};
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use std;
use amy;
use slog;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use msg::Msg;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use executor_status::ExecutorStatus;
use correlation_id::CorrelationId;

pub struct Executor<T: Encodable + Decodable + Send + Debug + Clone> {
    pid: Pid,
    node: NodeId,
    processes: HashMap<Pid, Box<Process<Msg=T>>>,
    thread_senders: HashMap<Pid, amy::Sender<Envelope<T>>>,
    tx: Sender<ExecutorMsg<T>>,
    rx: Receiver<ExecutorMsg<T>>,
    cluster_tx: Sender<ClusterMsg<T>>,
    logger: slog::Logger,
}

impl<T: Encodable + Decodable + Send + Debug + Clone> Executor<T> {
    pub fn new(node: NodeId,
               tx: Sender<ExecutorMsg<T>>,
               rx: Receiver<ExecutorMsg<T>>,
               cluster_tx: Sender<ClusterMsg<T>>,
               logger: slog::Logger) -> Executor<T> {
        let pid = Pid {
            group: Some("rabble".to_string()),
            name: "Executor".to_string(),
            node: node.clone()
        };
        Executor {
            pid: pid,
            node: node,
            processes: HashMap::new(),
            thread_senders: HashMap::new(),
            tx: tx,
            rx: rx,
            cluster_tx: cluster_tx,
            logger: logger.new(o!("component" => "executor"))
        }
    }

    /// Run the executor
    ///
    ///This call blocks the current thread indefinitely.
    pub fn run(mut self) {
        while let Ok(msg) = self.rx.recv() {
            match msg {
                ExecutorMsg::Envelope(envelope) => self.route(envelope),
                ExecutorMsg::Start(pid, process) => self.start(pid, process),
                ExecutorMsg::Stop(pid) => self.stop(pid),
                ExecutorMsg::RegisterSystemThread(pid, tx) => {
                    self.thread_senders.insert(pid, tx);
                },
                ExecutorMsg::GetStatus(correlation_id) => self.get_status(correlation_id),

                // Just return so the thread exits
                ExecutorMsg::Shutdown => return
            }
        }
    }

    fn get_status(&self, correlation_id: CorrelationId) {
        let status = ExecutorStatus {
            total_processes: self.processes.len(),
            system_threads: self.thread_senders.keys().cloned().collect()
        };
        let envelope = Envelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: Msg::ExecutorStatus(status),
            correlation_id: Some(correlation_id)
        };
        self.route_to_thread(envelope);
    }

    fn start(&mut self, pid: Pid, process: Box<Process<Msg=T>>) {
        self.processes.insert(pid, process);
    }

    fn stop(&mut self, pid: Pid) {
        self.processes.remove(&pid);
    }

    /// Route envelopes to local or remote processes
    ///
    /// Retrieve any envelopes from processes handling local messages and put them on either the
    /// executor or the cluster channel depending upon whether they are local or remote.
    ///
    /// Note that all envelopes sent to an executor are sent from the local cluster server and must
    /// be addressed to local processes.
    fn route(&mut self, envelope: Envelope<T>) {
        if self.node != envelope.to.node {
            let _ = self.cluster_tx.send(ClusterMsg::Envelope(envelope));
            return;
        }
        if let Err(envelope) = self.route_to_process(envelope) {
            self.route_to_thread(envelope);
        }
    }

    /// Route an envelope to a process if it exists on this node.
    ///
    /// Return Ok(()) if the process exists, Err(envelope) otherwise.
    fn route_to_process(&mut self, envelope: Envelope<T>) -> Result<(), Envelope<T>> {
        if let Some(process) = self.processes.get_mut(&envelope.to) {
            let Envelope {to, from, msg, correlation_id} = envelope;
            for envelope in process.handle(msg, from, correlation_id).drain(..) {
                if envelope.to.node == self.node {
                    // This won't ever fail because we hold a ref to both ends of the channel
                    self.tx.send(ExecutorMsg::Envelope(envelope)).unwrap();
                } else {
                    let _ = self.cluster_tx.send(ClusterMsg::Envelope(envelope));
                }
            }
            return Ok(());
        }
        Err(envelope)
    }

    /// Route an envelope to a system thread on this node
    fn route_to_thread(&self, envelope: Envelope<T>) {
        if let Some(tx) = self.thread_senders.get(&envelope.to) {
            let _ = tx.send(envelope);
        } else {
            warn!(self.logger, "Failed to find system thread: {}";
                  "pid" => envelope.to.to_string());
        }
    }
}
