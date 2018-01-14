use serde::{Serialize, Deserialize};
use std::mem;
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use amy;
use slog;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use msg::Msg;
use cluster::ClusterMsg;
use correlation_id::CorrelationId;
use super::{ExecutorStatus, ExecutorMsg, ExecutorMetrics};

pub struct Executor<T> {
    pid: Pid,
    node: NodeId,
    envelopes: Vec<Envelope<T>>,
    processes: HashMap<Pid, Box<Process<T>>>,
    service_senders: HashMap<Pid, amy::Sender<Envelope<T>>>,
    tx: Sender<ExecutorMsg<T>>,
    rx: Receiver<ExecutorMsg<T>>,
    cluster_tx: Sender<ClusterMsg<T>>,
    logger: slog::Logger,
    metrics: ExecutorMetrics
}

impl<'de, T: Serialize + Deserialize<'de> + Send + Debug + Clone> Executor<T> {
    pub fn new(node: NodeId,
               tx: Sender<ExecutorMsg<T>>,
               rx: Receiver<ExecutorMsg<T>>,
               cluster_tx: Sender<ClusterMsg<T>>,
               logger: slog::Logger) -> Executor<T> {
        let pid = Pid {
            group: Some("rabble".to_string()),
            name: "executor".to_string(),
            node: node.clone()
        };
        Executor {
            pid: pid,
            node: node,
            envelopes: Vec::new(),
            processes: HashMap::new(),
            service_senders: HashMap::new(),
            tx: tx,
            rx: rx,
            cluster_tx: cluster_tx,
            logger: logger.new(o!("component" => "executor")),
            metrics: ExecutorMetrics::new()
        }
    }

    /// Run the executor
    ///
    ///This call blocks the current thread indefinitely.
    pub fn run(mut self) {
        while let Ok(msg) = self.rx.recv() {
            match msg {
                ExecutorMsg::Envelope(envelope) => {
                    self.metrics.received_envelopes += 1;
                    self.route(envelope);
                },
                ExecutorMsg::Start(pid, process) => self.start(pid, process),
                ExecutorMsg::Stop(pid) => self.stop(pid),
                ExecutorMsg::RegisterService(pid, tx) => {
                    self.service_senders.insert(pid, tx);
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
            services: self.service_senders.keys().cloned().collect(),
            metrics: self.metrics.clone()
        };
        let envelope = Envelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: Msg::ExecutorStatus(status),
            correlation_id: Some(correlation_id)
        };
        self.route_to_service(envelope);
    }

    fn start(&mut self, pid: Pid, mut process: Box<Process<T>>) {
        let envelopes = process.init(self.pid.clone());
        self.processes.insert(pid, process);
        for envelope in envelopes {
                self.route(envelope);
        }
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
            self.cluster_tx.send(ClusterMsg::Envelope(envelope)).unwrap();
            return;
        }
        if let Err(envelope) = self.route_to_process(envelope) {
            self.route_to_service(envelope);
        }
    }

    /// Route an envelope to a process if it exists on this node.
    ///
    /// Return Ok(()) if the process exists, Err(envelope) otherwise.
    fn route_to_process(&mut self, envelope: Envelope<T>) -> Result<(), Envelope<T>> {
        if &envelope.to.name == "cluster_server" &&
            envelope.to.group.as_ref().unwrap() == "rabble"
        {
            self.cluster_tx.send(ClusterMsg::Envelope(envelope)).unwrap();
            return Ok(());
        }

        if let Some(process) = self.processes.get_mut(&envelope.to) {
            let Envelope {from, msg, correlation_id, ..} = envelope;
            process.handle(msg, from, correlation_id, &mut self.envelopes);
        } else {
            return Err(envelope);
        };

        // Take envelopes out of self temporarily so we don't get a borrowck error
        let mut envelopes = mem::replace(&mut self.envelopes, Vec::new());
        for envelope in envelopes.drain(..) {
            if envelope.to.node == self.node {
                // This won't ever fail because we hold a ref to both ends of the channel
                self.tx.send(ExecutorMsg::Envelope(envelope)).unwrap();
            } else {
                self.cluster_tx.send(ClusterMsg::Envelope(envelope)).unwrap();
            }
        }
        // Return the allocated vec back to self
        let _ = mem::replace(&mut self.envelopes, envelopes);
        Ok(())
    }

    /// Route an envelope to a service on this node
    fn route_to_service(&self, envelope: Envelope<T>) {
        if let Some(tx) = self.service_senders.get(&envelope.to) {
            tx.send(envelope).unwrap();
        } else {
            warn!(self.logger, "Failed to find service"; "pid" => envelope.to.to_string());
        }
    }
}
