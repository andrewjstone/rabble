use rustc_serialize::{Encodable, Decodable};
use std::fmt::Debug;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use amy;
use slog;
use time::{Duration, PreciseTime};
use ferris::{Wheel, CopyWheel, Resolution};
use envelope::Envelope;
use pid::Pid;
use process::Process;
use node_id::NodeId;
use msg::Msg;
use executor_msg::ExecutorMsg;
use cluster_msg::ClusterMsg;
use correlation_id::CorrelationId;
use status::{StatusVal, TimeUnit};
use hdrsample;
use histogram::Histogram;

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

    // Status Information
    envelopes: u64,
    starts: u64,
    stops: u64,
    service_registrations: u64,
    get_status_msgs: u64,
    ticks: u64,
    time_between_ticks: hdrsample::Histogram<u64>,
    route_to_local_process: hdrsample::Histogram<u64>,
    route_to_remote_process: hdrsample::Histogram<u64>,
    route_to_service: hdrsample::Histogram<u64>

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

            // Status specific fields
            envelopes: 0,
            starts: 0,
            stops: 0,
            service_registrations: 0,
            get_status_msgs: 0,
            ticks: 0,
            time_between_ticks: hdrsample::Histogram::<u64>::new(3).unwrap(),
            route_to_local_process: hdrsample::Histogram::<u64>::new(3).unwrap(),
            route_to_remote_process: hdrsample::Histogram::<u64>::new(3).unwrap(),
            route_to_service: hdrsample::Histogram::<u64>::new(3).unwrap()
        }
    }

    fn create_status_table(&self) -> HashMap<String, StatusVal> {
        let mut table = HashMap::new();
        table.insert("envelopes".to_string(),
                     StatusVal::Int(self.envelopes));
        table.insert("starts".to_string(),
                     StatusVal::Int(self.starts));
        table.insert("stops".to_string(),
                     StatusVal::Int(self.stops));
        table.insert("service_registrations".to_string(),
                     StatusVal::Int(self.service_registrations));
        table.insert("get_status_msgs".to_string(),
                     StatusVal::Int(self.get_status_msgs));
        table.insert("ticks".to_string(),
                     StatusVal::Int(self.ticks));
        table.insert("processes".to_string(),
                     StatusVal::Int(self.processes.len() as u64));
        table.insert("services".to_string(),
                     StatusVal::StringSet(
                         self.service_senders
                             .keys()
                             .cloned()
                             .map(|pid| pid.to_string()).collect()));
        table.insert("time_between_ticks".to_string(),
                     StatusVal::Histogram(
                         TimeUnit::Milliseconds,
                         Histogram::from(self.time_between_ticks.clone())));
        table.insert("route_to_local_process".to_string(),
                     StatusVal::Histogram(
                         TimeUnit::Microseconds,
                         Histogram::from(self.route_to_local_process.clone())));
        table.insert("route_to_remote_process".to_string(),
                     StatusVal::Histogram(
                         TimeUnit::Nanoseconds,
                         Histogram::from(self.route_to_remote_process.clone())));
        table.insert("route_to_service".to_string(),
                     StatusVal::Histogram(
                         TimeUnit::Nanoseconds,
                         Histogram::from(self.route_to_service.clone())));
        
        table
    }

    /// Run the executor
    ///
    ///This call blocks the current thread indefinitely.
    pub fn run(mut self) {
        let mut last_tick: Option<PreciseTime> = None;
        while let Ok(msg) = self.rx.recv() {
            match msg {
                ExecutorMsg::Envelope(envelope) => {
                    self.envelopes += 1;
                    self.route(envelope)
                },
                ExecutorMsg::Start(pid, process) => {
                    self.starts += 1;
                    self.start(pid, process)
                },
                ExecutorMsg::Stop(pid) => {
                    self.stops += 1;
                    self.stop(pid)
                },
                ExecutorMsg::RegisterService(pid, tx) => {
                    self.service_registrations += 1;
                    self.service_senders.insert(pid, tx);
                },
                ExecutorMsg::GetStatus(correlation_id) => {
                    self.get_status_msgs += 1;
                    self.get_status(correlation_id)
                },
                ExecutorMsg::Tick => {
                    if last_tick.is_some() {
                        let _ = self.time_between_ticks.record(
                            last_tick.unwrap().to(PreciseTime::now()).num_milliseconds() as u64);
                    }
                    last_tick = Some(PreciseTime::now());
                    self.ticks += 1;
                    self.tick()
                },

                // Just return so the thread exits
                ExecutorMsg::Shutdown => return
            }
        }
    }

    fn get_status(&mut self, correlation_id: CorrelationId) {
        let envelope = Envelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: Msg::Status(self.create_status_table()),
            correlation_id: Some(correlation_id)
        };
        self.route_to_service(envelope);
    }

    fn start(&mut self, pid: Pid, mut process: Box<Process<Msg=T>>) {
        let envelopes = process.init(self.pid.clone());
        self.processes.insert(pid, process);
        for envelope in envelopes {
            if envelope.to == self.pid {
                handle_executor_envelope(&mut self.timer_wheel, envelope, &mut self.logger);
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
            let start = PreciseTime::now();
            self.cluster_tx.send(ClusterMsg::Envelope(envelope)).unwrap();
            let _ = self.route_to_remote_process.record(
                start.to(PreciseTime::now()).num_nanoseconds().unwrap() as u64);
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
        let start = PreciseTime::now();
        if let Some(process) = self.processes.get_mut(&envelope.to) {
            let Envelope {from, msg, correlation_id, ..} = envelope;
            for envelope in process.handle(msg, from, correlation_id).drain(..) {
                if envelope.to == self.pid {
                    handle_executor_envelope(&mut self.timer_wheel, envelope, &mut self.logger);
                    continue;
                }
                if envelope.to.node == self.node {
                    // This won't ever fail because we hold a ref to both ends of the channel
                    self.tx.send(ExecutorMsg::Envelope(envelope)).unwrap();
                } else {
                    self.cluster_tx.send(ClusterMsg::Envelope(envelope)).unwrap();
                }
            }
            let _ = self.route_to_local_process.record(
                start.to(PreciseTime::now()).num_microseconds().unwrap() as u64);
            return Ok(());
        }
        Err(envelope)
    }

    /// Route an envelope to a service on this node
    fn route_to_service(&mut self, envelope: Envelope<T>) {
        let start = PreciseTime::now();
        if let Some(tx) = self.service_senders.get(&envelope.to) {
            tx.send(envelope).unwrap();
            let _ = self.route_to_service.record(
                start.to(PreciseTime::now()).num_nanoseconds().unwrap() as u64);
        } else {
            warn!(self.logger, "Failed to find service"; "pid" => envelope.to.to_string());
        }
    }
}

fn handle_executor_envelope<T>(timer_wheel: &mut CopyWheel<(Pid, Option<CorrelationId>)>,
                               envelope: Envelope<T>,
                               logger: &mut slog::Logger)
   where T: Encodable + Decodable + Send + Debug + Clone
{
    let Envelope {from, msg, correlation_id, ..} = envelope;
    match msg {
        Msg::StartTimer(time_in_ms) => timer_wheel.start((from, correlation_id),
                                                         Duration::milliseconds(time_in_ms as i64)),
        Msg::CancelTimer(correlation_id) => timer_wheel.stop((from, correlation_id)),
        _ => error!(logger, "Invalid message sent to executor";
                    "from" => from.to_string(), "msg" => format!("{:?}", msg))
    }
}
