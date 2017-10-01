extern crate rabble;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate rand;
extern crate amy;

use std::time;
use std::thread;
use std::sync::mpsc::channel;
use slog::Drain;
use clap::{Arg, App};
use rand::Rng;
use rabble::{Msg, Pid, NodeId, Envelope, CorrelationId, Process, Processes, Scheduler};

fn main() {
    let matches = App::new("Scheduler Benchmarker")
        .version(crate_version!())
        .author("Andrew Stone <andrew.j.stone.1@gmail.com>")
        .about("Benchmark lightweight process scheduling with varying numbers of schedulers, \
        processes, and concurrent senders")
        .arg(Arg::with_name("processes")
             .short("p")
             .long("processes")
             .help("Set the number of total processes")
             .required(false)
             .default_value("1000")
             .takes_value(true))
        .arg(Arg::with_name("schedulers")
             .short("s")
             .long("schedulers")
             .help("Set the number of schedulers")
             .required(false)
             .default_value("1")
             .takes_value(true))
        .arg(Arg::with_name("senders")
             .short("t")
             .long("senders")
             .help("Set the number of concurrent sending threads")
             .required(false)
             .default_value("2")
             .takes_value(true))
        .get_matches();

    let num_processes = value_t!(matches, "processes", u32).unwrap_or_else(|e| e.exit());
    let num_schedulers = value_t!(matches, "schedulers", u32).unwrap_or_else(|e| e.exit());
    let num_senders = value_t!(matches, "senders", u32).unwrap_or_else(|e| e.exit());
    let duration = time::Duration::from_secs(5);
    let pids = pids(num_processes);
    let logger = logger();
    let mut processes = Processes::<TestMsg>::new(logger.clone());
    spawn(&pids, &mut processes);

    let scheduler_pid = Pid { name: "scheduler1".to_owned(), group: None, node: node_id() };
    // Fake out the cluster server as we aren't using multiple nodes
    let (cluster_tx, _) = channel();
    let scheduler = Scheduler::new(scheduler_pid, processes.clone(), cluster_tx.clone(), logger.clone());

    let mut handles = Vec::new();

    handles.push(thread::Builder::new().name("scheduler1".to_owned()).spawn(move || {
        scheduler.run()
    }).unwrap());

    handles.push(thread::Builder::new().name("Sender1".to_owned()).spawn(move || {
        send(pids.clone(), &mut processes);
    }).unwrap());

    for h in handles {
        h.join().unwrap()
    }
}

fn spawn(pids: &[Pid], processes: &mut Processes<TestMsg>) {
    for pid in pids {
        processes.spawn(pid.clone(),
                        Box::new(Counter::new(pid.clone())) as Box<Process<TestMsg>>).unwrap();
    }
}

fn send(pids: Vec<Pid>, processes: &mut Processes<TestMsg>) {
    let poller = amy::Poller::new().unwrap();
    let mut registrar = poller.get_registrar().unwrap();
    let pid = pid("sender1");
    let (tx, rx) = registrar.channel().unwrap();
    processes.register_service(pid.clone(), tx).unwrap();
    let mut rng = rand::thread_rng();
    let count = 1000000;
    for _ in 0..count {
        let to = rng.choose(&pids).unwrap().clone();
        let envelope = Envelope::new(to, pid.clone(), Msg::User(TestMsg::Request), None);
        processes.send(envelope).unwrap();
    }

    let mut received = 0;
    loop {
        if let Ok(_) = rx.try_recv() {
            received +=1;
        }
        if received == count {
            break;
        }
    }

    println!("shutdown");
    processes.shutdown();
}

fn logger() -> slog::Logger {
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!())
}

fn node_id() -> NodeId {
    NodeId {
        name: "node1".to_owned(),
        addr: "127.0.0.1:5000".to_owned()
    }
}

fn pid(name: &str) -> Pid {
    Pid {
        name: name.to_owned(),
        group: None,
        node: node_id()
    }
}

fn pids(n: u32) -> Vec<Pid> {
    (0..n).map(|i| Pid {
        name: i.to_string(),
        group: None,
        node: node_id()
    }).collect()
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
