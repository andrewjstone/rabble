use rustc_serialize::{Encodable, Decodable};
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use amy;
use slog;
use time::Duration;
use ferris::{Wheel, CopyWheel, Resolution};
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use msg::Msg;
use cluster::ClusterMsg;
use correlation_id::CorrelationId;
use metrics::Metrics;
use super::{ExecutorStatus, ExecutorMetrics, ExecutorMsg};

pub struct Executor<T: Encodable + Decodable + Send + Debug + Clone> {
    pid: Pid,
    node: NodeId,
    processes: HashMap<Pid, Box<Process<Msg=T>>>,
    service_senders: HashMap<Pid, amy::Sender<Envelope<T>>>,
    tx: Sender<ExecutorMsg<T>>,
    rx: Receiver<ExecutorMsg<T>>,
    cluster_tx: Sender<ClusterMsg<T>>,
    timer_wheel: CopyWheel<(Pid, Option<CorrelationId>)>,
    logger: slog::Logger,
    metrics: ExecutorMetrics
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
        let envelope = Envelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: Msg::ExecutorStatus(status),
            correlation_id: Some(correlation_id)
        };
        self.route_to_service(envelope);
    }

    fn start(&mut self, pid: Pid, mut process: Box<Process<Msg=T>>) {
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
        let envelopes: Vec<_> = if let Some(process) = self.processes.get_mut(&envelope.to) {
            let Envelope {from, msg, correlation_id, ..} = envelope;
            process.handle(msg, from, correlation_id).drain(..).collect()
        } else {
            return Err(envelope);
        };

        for envelope in envelopes {
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

    fn handle_executor_envelope(&mut self, envelope: Envelope<T>) {
        let Envelope {from, msg, correlation_id, ..} = envelope;
        match msg {
            Msg::StartTimer(time_in_ms) => {
                self.timer_wheel.start((from, correlation_id),
                                       Duration::milliseconds(time_in_ms as i64));
                self.metrics.timers_started += 1;
            },
            Msg::CancelTimer(correlation_id) => {
                self.timer_wheel.stop((from, correlation_id));
                self.metrics.timers_cancelled += 1;
            }
            Msg::GetMetrics => self.send_metrics(from, correlation_id),
            _ => error!(self.logger, "Invalid message sent to executor";
                        "from" => from.to_string(), "msg" => format!("{:?}", msg))
        }
    }

    fn send_metrics(&mut self, from: Pid, correlation_id: Option<CorrelationId>) {
        self.metrics.processes = self.processes.len() as i64;
        self.metrics.services = self.service_senders.len() as i64;
        let envelope = Envelope {
            to: from,
            from: self.pid.clone(),
            msg: Msg::Metrics(self.metrics.data()),
            correlation_id: correlation_id
        };
        self.route(envelope);
    }
}

