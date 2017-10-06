#![feature(test)]

extern crate test;
extern crate rabble;
extern crate amy;
#[macro_use]
extern crate slog;
extern crate slog_stdlog;
#[macro_use]
extern crate serde_derive;


use test::Bencher;
use slog::{Logger, DrainExt};
use std::sync::mpsc::channel;
use std::thread::spawn;
use rabble::{Msg, Pid, NodeId, Envelope, CorrelationId, Executor, Process};

#[bench]
fn create_pid(b: &mut Bencher) {
    b.iter(|| {
        pid("counter1")
    })
}

#[bench]
fn create_process(b: &mut Bencher) {
    b.iter(|| {
        Box::new(Counter::new(pid("counter1"))) as Box<Process<TestMsg>>
    })
}

#[bench]
fn create_envelope(b: &mut Bencher) {
    b.iter(|| {
        let msg = Msg::User(TestMsg::Inc);
        Envelope::new(pid("counter1"), pid("counter2"), msg, None)
    })
}

#[bench]
fn start_process(b: &mut Bencher) {
    let (tx, rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let mut executor = Executor::new(node_id(), tx.clone(), rx, cluster_tx, logger());
    b.iter(|| {
        let pid = pid("counter1");
        let process = Box::new(Counter::new(pid.clone())) as Box<Process<TestMsg>>;
        executor.start(pid, process)
    })
}

#[bench]
fn route(b: &mut Bencher) {
    let (tx, rx) = channel();
    let (cluster_tx, cluster_rx) = channel();
    let mut executor = Executor::new(node_id(), tx.clone(), rx, cluster_tx, logger());
    let pid1 = pid("counter1");
    let process = Box::new(Counter::new(pid1.clone())) as Box<Process<TestMsg>>;
    executor.start(pid1, process);
    b.iter(|| {
        let msg = Msg::User(TestMsg::Inc);
        executor.route(Envelope::new(pid("counter1"), pid("counter2"), msg, None))
    })
}

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
              _correlation_id: Option<CorrelationId>,
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
