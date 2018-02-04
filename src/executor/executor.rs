use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use std::time::Instant;
use slog;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use cluster::ClusterMsg;
use super::{ExecutorStatus, ExecutorMsg, ExecutorMetrics, ExecutorTerminal};
use channel;

pub struct Executor<T> {
    node: NodeId,
    processes: HashMap<Pid, Box<Process<T>>>,
    service_senders: HashMap<Pid, Box<channel::Sender<Envelope<T>>>>,
    terminal: ExecutorTerminal<T>,
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
            node: node,
            processes: HashMap::new(),
            service_senders: HashMap::new(),

            // pid is just a placeholder pid, since we change it when we call a process
            terminal: ExecutorTerminal::new(pid, tx, cluster_tx.clone()),
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
                    let start = Instant::now();
                    self.route(envelope);
                    self.metrics.route_envelope_ns.0 += duration(start);

                },
                ExecutorMsg::Start(pid, process) => self.start(pid, process),
                ExecutorMsg::Stop(pid) => self.stop(pid),
                ExecutorMsg::RegisterService(pid, tx) => {
                    self.service_senders.insert(pid, tx);
                },
                ExecutorMsg::GetStatus(tx) => self.get_status(tx),
                // Just return so the thread exits
                ExecutorMsg::Shutdown => return
            }
        }
    }

    fn get_status(&self, tx: Box<channel::Sender<ExecutorStatus>>) {
        let status = ExecutorStatus {
            total_processes: self.processes.len(),
            services: self.service_senders.keys().cloned().collect(),
            metrics: self.metrics.clone()
        };
        let _ = tx.send(status);
    }

    // Public only for benchmarking
    #[doc(hidden)]
    pub fn start(&mut self, pid: Pid, mut process: Box<Process<T>>) {
        self.terminal.set_pid(pid.clone());
        process.init(&mut self.terminal);
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
    ///
    /// Public only for benchmarking
    #[doc(hidden)]
    pub fn route(&mut self, envelope: Envelope<T>) {
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
            let Envelope {to, from, msg} = envelope;
            self.terminal.set_pid(to);
            process.handle(msg, from, &mut self.terminal);
        } else {
            return Err(envelope);
        };

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

/// Return the number of nanoseconds since start
///
/// Ignore any possible overflow as the number of seconds is guaranteed to be small (likely 0)
#[inline]
fn duration(start: Instant) -> u64 {
    let elapsed = start.elapsed();
    elapsed.as_secs()*1000000000 + elapsed.subsec_nanos() as u64
}
