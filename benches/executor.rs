#[macro_use]
extern crate criterion;

extern crate rabble;
extern crate amy;
#[macro_use]
extern crate slog;
extern crate slog_stdlog;
#[macro_use]
extern crate serde_derive;


use criterion::Criterion;
use slog::{Logger, DrainExt};
use std::sync::mpsc::channel;
use rabble::{Msg, Pid, NodeId, Envelope, Executor, Process};

fn create_pid(c: &mut Criterion) {
    c.bench_function("create Pid", |b| b.iter(|| pid("counter1")));
}

fn create_process(c: &mut Criterion) {
	c.bench_function("create Process", |b| b.iter(|| {
		Box::new(Counter::new(pid("counter1"))) as Box<Process<TestMsg>>
	}));
}

fn create_envelope(c: &mut Criterion) {
    c.bench_function("create Envelope", |b| b.iter(|| {
        let msg = Msg::User(TestMsg::Inc);
        Envelope::new(pid("counter1"), pid("counter2"), msg)
    }));
}

fn start_process(c: &mut Criterion) {
    let (tx, rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let mut executor = Executor::new(node_id(), tx.clone(), rx, cluster_tx, logger());
    c.bench_function("start process", |b| b.iter(|| {
        let pid = pid("counter1");
        let process = Box::new(Counter::new(pid.clone())) as Box<Process<TestMsg>>;
        executor.start(pid, process)
    }));
}

fn route(c: &mut Criterion) {
    let (tx, rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let mut executor = Executor::new(node_id(), tx.clone(), rx, cluster_tx, logger());
    let pid1 = pid("counter1");
    let process = Box::new(Counter::new(pid1.clone())) as Box<Process<TestMsg>>;
    executor.start(pid1, process);
    c.bench_function("route envelope", |b| b.iter(|| {
        let msg = Msg::User(TestMsg::Inc);
        executor.route(Envelope::new(pid("counter1"), pid("counter2"), msg))
    }));
}

criterion_group!(benches, create_pid, create_process, create_envelope, start_process, route);
criterion_main!(benches);

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TestMsg {
    Inc
}

struct Counter {
    pid: Pid,
    count: u64
}

impl Counter {
    fn new(pid: Pid) -> Counter {
        Counter {
            pid: pid,
            count: 0
        }
    }
}

impl Process<TestMsg> for Counter {
    fn handle(&mut self,
              _msg: Msg<TestMsg>,
              _from: Pid,
              output: &mut Vec<Envelope<TestMsg>>)
    {
        self.count += 1;
    }
}

fn pid(name: &str) -> Pid {
    Pid {
        name: name.to_owned(),
        group: None,
        node: node_id()
    }
}

fn node_id() -> NodeId {
    NodeId {
        name: "node1".to_owned(),
        addr: "127.0.0.1:5000".to_owned()
    }
}

fn logger() -> Logger {
    Logger::root(slog_stdlog::StdLog.fuse(), o!())
}
