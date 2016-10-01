use rustc_serialize::{Encodable, Decodable};
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use std;
use amy;
use slog;
use envelope::{Envelope, SystemEnvelope, ProcessEnvelope};
use pid::Pid;
use process::Process;
use node_id::NodeId;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use executor_status::ExecutorStatus;
use system_msg::SystemMsg;
use correlation_id::CorrelationId;

pub struct Executor<T: Encodable + Decodable + Send, U: Debug> {
    pid: Pid,
    node: NodeId,
    processes: HashMap<Pid, Box<Process<Msg=T, SystemUserMsg=U>>>,
    system_senders: HashMap<Pid, amy::Sender<SystemEnvelope<U>>>,
    tx: Sender<ExecutorMsg<T, U>>,
    rx: Receiver<ExecutorMsg<T, U>>,
    cluster_tx: Sender<ClusterMsg<T>>,
    logger: slog::Logger,
}

impl<T: Encodable + Decodable + Send, U: Debug> Executor<T, U> {
    pub fn new(node: NodeId,
               tx: Sender<ExecutorMsg<T, U>>,
               rx: Receiver<ExecutorMsg<T, U>>,
               cluster_tx: Sender<ClusterMsg<T>>,
               logger: slog::Logger) -> Executor<T, U> {
        let pid = Pid {
            group: Some("rabble".to_string()),
            name: "Executor".to_string(),
            node: node.clone()
        };
        Executor {
            pid: pid,
            node: node,
            processes: HashMap::new(),
            system_senders: HashMap::new(),
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
                ExecutorMsg::User(envelope) => self.route(envelope),
                ExecutorMsg::Start(pid, process) => self.start(pid, process),
                ExecutorMsg::Stop(pid) => self.stop(pid),
                ExecutorMsg::RegisterSystemThread(pid, tx) => {
                    self.system_senders.insert(pid, tx);
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
            system_threads: self.system_senders.keys().cloned().collect()
        };
        let envelope = SystemEnvelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: SystemMsg::ExecutorStatus(status),
            correlation_id: Some(correlation_id)
        };
        self.route_to_thread(envelope);
    }

    fn start(&mut self, pid: Pid, process: Box<Process<Msg=T, SystemUserMsg=U>>) {
        self.processes.insert(pid, process);
    }

    fn stop(&mut self, pid: Pid) {
        self.processes.remove(&pid);
    }

    /// Route envelopes to local or remote processes
    ///
    /// Retrieve any envelopes from processes handling local messages and put them on either the
    /// executor or the cluster channel depending upon whether they are local or remote.
    fn route(&mut self, envelope: Envelope<T, U>) {
        match envelope {
            Envelope::Process(process_envelope) => self.route_to_process(process_envelope),
            Envelope::System(system_envelope) => self.route_to_thread(system_envelope)
        }
    }

    fn route_to_process(&mut self, envelope: ProcessEnvelope<T>) {
        let ProcessEnvelope {to, from, msg, correlation_id} = envelope;
        if let Some(process) = self.processes.get_mut(&to) {
            for envelope in process.handle(msg, from, correlation_id).drain(..) {
                if envelope.to().node == self.node {
                    // This won't ever fail because we hold a ref to both ends of the channel
                    self.tx.send(ExecutorMsg::User(envelope)).unwrap();
                } else {
                    if let Envelope::Process(process_envelope) = envelope {
                        // Return if the cluster server thread has exited
                        // The system is shutting down.
                        if let Err(_) = self.cluster_tx.send(ClusterMsg::User(process_envelope)) {
                            return;
                        }
                    } else {
                        // TODO: Log error. We are trying to send a SystemEnvelope remotely
                    }
                }
            }
        }
    }

    fn route_to_thread(&self, envelope: SystemEnvelope<U>) {
        if let Some(tx) = self.system_senders.get(&envelope.to) {
            tx.send(envelope).unwrap();
        } else {
            // TODO: Logging
        }
    }
}
