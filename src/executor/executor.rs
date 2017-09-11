use serde::{Serialize, Deserialize};
use std::mem;
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use amy;
use slog;
use time::Duration; use ferris::{Wheel, CopyWheel, Resolution};
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use msg::Msg;
use cluster::ClusterMsg;
use correlation_id::CorrelationId;
use metrics::Metrics;
use super::{ExecutorStatus, ExecutorMetrics, ExecutorMsg};
use rabble_msgs::Request;

pub struct Executor {
    pid: Pid,
    node: NodeId,
    envelopes: Vec<Envelope>,
    processes: HashMap<Pid, Box<Process>>,
    service_senders: HashMap<Pid, amy::Sender<Envelope>>,
    tx: Sender<ExecutorMsg>,
    rx: Receiver<ExecutorMsg>,
    cluster_tx: Sender<ClusterMsg>,
    timer_wheel: CopyWheel<(Pid, Option<CorrelationId>)>,
    logger: slog::Logger,
    metrics: ExecutorMetrics
}

impl Executor {
    pub fn new(node: NodeId,
               tx: Sender<ExecutorMsg>,
               rx: Receiver<ExecutorMsg>,
               cluster_tx: Sender<ClusterMsg>,
               logger: slog::Logger) -> Executor {
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
            timer_wheel: CopyWheel::new(vec![Resolution::TenMs, Resolution::Sec, Resolution::Min]),
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
                ExecutorMsg::Tick => self.tick(),

                // Just return so the thread exits
                ExecutorMsg::Shutdown => return
            }
        }
    }

    fn get_status(&self, correlation_id: CorrelationId) {
        let status = ExecutorStatus {
            total_processes: self.processes.len(),
            services: self.service_senders.keys().cloned().collect()
        };
        let to = correlation_id.pid.clone();
        let from = self.pid.clone();
        Envelope::new(to, from, Msg::ExecutorStatus(status), Some(correlation_id));
        self.route_to_service(envelope);
    }

    fn start(&mut self, pid: Pid, mut process: Box<Process>) {
        let envelopes = process.init(self.pid.clone());
        self.processes.insert(pid, process);
        for envelope in envelopes {
            if envelope.to == self.pid {
                self.handle_executor_envelope(envelope);
            } else {
                self.route(envelope);
            }
        }
    }

    fn stop(&mut self, pid: Pid) {
        self.processes.remove(&pid);
    }

    fn tick(&mut self) {
        for (pid, c_id) in self.timer_wheel.expire() {
            let envelope = Envelope::new(pid, self.pid.clone(), Msg::Timeout, c_id);
            let _ = self.route_to_process(envelope);
        }
    }

    /// Route envelopes to local or remote processes
    ///
    /// Retrieve any envelopes from processes handling local messages and put them on either the
    /// executor or the cluster channel depending upon whether they are local or remote.
    ///
    /// Note that all envelopes sent to an executor are sent from the local cluster server and must
    /// be addressed to local processes.
    fn route(&mut self, envelope: Envelope) {
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
    fn route_to_process(&mut self, envelope: Envelope) -> Result<(), Envelope> {
        if envelope.to == self.pid {
            self.handle_executor_envelope(envelope);
            return Ok(());
        }

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
            if envelope.to == self.pid {
                self.handle_executor_envelope(envelope);
                continue;
            }
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
    fn route_to_service(&self, envelope: Envelope) {
        if let Some(tx) = self.service_senders.get(&envelope.to) {
            tx.send(envelope).unwrap();
        } else {
            warn!(self.logger, "Failed to find service"; "pid" => envelope.to.to_string());
        }
    }

    fn handle_executor_envelope(&mut self, envelope: Envelope) {
        let Envelope {from, msg, correlation_id, ..} = envelope;
        decode!(msg, concrete, err {
            Request => {
                match concrete {
                    Request::GetMetrics => self.send_metrics(from, correlation_id),
                    _ => error!(self.logger, "Invalid rabble_msgs::Request sent to executor";
                                "from" => from.to_string(), "msg_id" => format!("{:?}", msg.id))
                }
            }
            _ => {
                error!(self.logger, "Failed to decode executor message";
                       "error" => err.to_string(),
                       "from" => from.to_string(),
                       "msg_id" => format!("{:?}", msg.id));

            }
        });
    }

    fn send_metrics(&mut self, from: Pid, correlation_id: Option<CorrelationId>) {
        self.metrics.processes = self.processes.len() as i64;
        self.metrics.services = self.service_senders.len() as i64;
        let msg = Msg::Metrics(self.metrics.data());
        let envelope = Envelope::new(from, self.pid.clone(), msg, correlation_id);
        self.route(envelope);
    }
}

