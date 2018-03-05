use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use std::collections::HashMap;
use std::time::Instant;
use std::vec::Drain;
use slog;
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use super::{ExecutorStatus, ExecutorMetrics, ExecutorTerminal};
use channel::Sender;

pub struct Executor<T> {
    node: NodeId,
    processes: HashMap<Pid, Box<Process<T>>>,
    service_senders: HashMap<Pid, Box<Sender<Envelope<T>>>>,
    terminal: ExecutorTerminal<T>,
    logger: slog::Logger,
    remote: Vec<Envelope<T>>,
    metrics: ExecutorMetrics
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone + Send + 'static> Executor<T> {
    pub fn new(node: NodeId, logger: slog::Logger) -> Executor<T> {
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
            terminal: ExecutorTerminal::new(pid),
            logger: logger.new(o!("component" => "executor")),
            remote: Vec::new(),
            metrics: ExecutorMetrics::new()
        }
    }

    pub fn spawn(&mut self, pid: Pid, mut process: Box<Process<T>>) {
        self.terminal.set_pid(pid.clone());
        process.init(&mut self.terminal);
        self.processes.insert(pid, process);
    }

    pub fn stop(&mut self, pid: Pid) {
        self.processes.remove(&pid);
    }

    pub fn process_timeouts(&mut self) {
        self.terminal.process_timeouts();
        self.route_pending();
    }

    pub fn register_service(&mut self, pid: Pid, tx: Box<Sender<Envelope<T>>>) {
        self.service_senders.insert(pid, tx);
    }

    pub fn get_status(&self) -> ExecutorStatus {
        ExecutorStatus {
            total_processes: self.processes.len(),
            services: self.service_senders.keys().cloned().collect(),
            metrics: self.metrics.clone()
        }
    }

    /// Send an envelope to a process and then also send any output local envelopes until there are
    /// no more envelopes.
    pub fn send(&mut self, envelope: Envelope<T>) {
        self.route(envelope);
        self.route_pending();
    }

    /// Return a Drain iterator for all remote envelopes that need to be sent.
    pub fn remote_envelopes(&mut self) -> Drain<Envelope<T>> {
        self.remote.drain(..)
    }

    /// Route all pending envelopes
    fn route_pending(&mut self) {
        let mut done = false;
        while !done {
            done = true;
            let pending: Vec<_> = self.terminal.pending().collect();
            for envelope in pending {
                done = false;
                self.route(envelope)
            }
        }
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
        self.metrics.received_envelopes += 1;
        let start = Instant::now();
        if self.node != envelope.to.node {
            self.remote.push(envelope);
        } else {
            if let Err(envelope) = self.route_to_process(envelope) {
                self.route_to_service(envelope);
            }
        }
        self.metrics.route_envelope_ns.0 += duration(start);
    }

    /// Route an envelope to a process if it exists on this node.
    ///
    /// Return Ok(()) if the process exists, Err(envelope) otherwise.
    fn route_to_process(&mut self, envelope: Envelope<T>) -> Result<(), Envelope<T>> {
        if &envelope.to.name == "cluster_server" &&
            envelope.to.group.as_ref().unwrap() == "rabble"
        {
            self.remote.push(envelope);
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
