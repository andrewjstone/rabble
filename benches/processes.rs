#![feature(test)]

extern crate test;
extern crate rabble;
extern crate amy;
#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;
#[macro_use]
extern crate serde_derive;

use test::Bencher;
use slog::Drain;
use rabble::{Pid, NodeId, CorrelationId, Envelope, Msg, Process, Processes};

//
// Microbenchmarks of different operations on Processes structure
//

#[bench]
fn spawn(b: &mut Bencher) {
    let processes = processes();
    let pid = pid("counter1");
    b.iter(|| processes.spawn(pid.clone(), Box::new(Counter::new(pid.clone())) as Box<Process<TestMsg>>));
}

#[bench]
fn send(b: &mut Bencher) {
    let mut processes = processes();
    let pid = pid("counter1");
    processes.spawn(pid.clone(), Box::new(Counter::new(pid.clone())) as Box<Process<TestMsg>>).unwrap();
    // Note that there is no scheduler here. This only measures uncontended send calls.
    b.iter(|| processes.send(Envelope::new(pid.clone(), pid.clone(), Msg::User(TestMsg::Request), None)))
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

fn processes() -> Processes<TestMsg> {
    Processes::<TestMsg>::new(logger())
}

fn logger() -> slog::Logger {
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, slog::Level::Critical).fuse();
    slog::Logger::root(drain, o!())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TestMsg {
    Request,
    Reply
}

struct Counter {
    pid: Pid,
    count: u64
}

impl Counter {
    pub fn new(pid: Pid) -> Counter {
        Counter {
            pid: pid,
            count: 0
        }
    }
}

/// This process receives a message, increments its counter and replies to its sender if the message
/// was a request.
impl Process<TestMsg> for Counter {
    fn handle(&mut self,
              msg: Msg<TestMsg>,
              from: Pid,
              _: Option<CorrelationId>,
              output: &mut Vec<Envelope<TestMsg>>) {
        self.count += 1;
        match msg {
            Msg::User(TestMsg::Request) => {
                let msg = Msg::User(TestMsg::Reply);
                output.push(Envelope::new(from, self.pid.clone(), msg, None));
            }
            _ => ()
        }
    }
}
