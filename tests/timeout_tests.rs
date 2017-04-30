extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;

#[macro_use]
extern crate slog;
extern crate slog_stdlog;
extern crate slog_envlogger;
extern crate slog_term;
extern crate log;
extern crate time;

mod utils;

use std::{str};
use std::net::TcpStream;
use std::sync::mpsc;
use amy::Sender;
use time::Duration;

use utils::messages::*;
use utils::api_server;
use utils::{
    wait_for,
    send
};

use rabble::{
    Pid,
    Process,
    Envelope,
    Msg,
    MsgpackSerializer,
    Serialize,
    Node,
    NodeId,
    CorrelationId
};

const CLUSTER_SERVER_IP: &'static str = "127.0.0.1:11001";
const API_SERVER_IP: &'static str = "127.0.0.1:12001";

#[test]
fn connection_timeout() {
    let node_id = NodeId {name: "node1".to_string(), addr: CLUSTER_SERVER_IP.to_string()};
    let (node, mut handles) = rabble::rouse::<RabbleUserMsg>(node_id, None);

    let (service_pid, service_tx, service_handle) = api_server::start(node.clone());
    handles.push(service_handle);

    run_client_operation_against_nonexistant_pid_and_wait_for_timeout(node.id.clone());

    shutdown(node, service_pid, service_tx);

    for h in handles {
        h.join().unwrap();
    }
}

struct TestProcess {
    pid: Pid,
    executor_pid: Option<Pid>,
    output: Vec<Envelope<()>>,

    /// Don't do this in production!!!
    /// This is only hear to signal to the test that it has received a message.
    tx: mpsc::Sender<()>
}

impl Process for TestProcess {
    type Msg = ();

    fn init(&mut self, executor_pid: Pid) -> Vec<Envelope<()>> {
        self.executor_pid = Some(executor_pid);
        // Start a timer with a 100ms timeout and no correlation id. We don't need one
        // since there is only one timer in this example
        vec![Envelope::new(self.executor_pid.as_ref().unwrap().clone(),
                           self.pid.clone(),
                           Msg::StartTimer(100),
                           None)]
    }

    fn handle(&mut self,
              msg: Msg<()>,
              from: Pid,
              correlation_id: Option<CorrelationId>) -> &mut Vec<Envelope<()>>
    {
        assert_eq!(from, *self.executor_pid.as_ref().unwrap());
        assert_eq!(msg, Msg::Timeout);
        assert_eq!(correlation_id, None);
        self.tx.send(()).unwrap();
        &mut self.output
    }
}

#[test]
fn process_timeout() {
    let node_id = NodeId {name: "node1".to_string(), addr: "127.0.0.1:11002".to_string()};
    let (node, handles) = rabble::rouse::<()>(node_id.clone(), None);

    let pid = Pid {
        name: "some-process".to_string(),
        group: None,
        node: node_id
    };

    let (tx, rx) = mpsc::channel();

    let process = TestProcess {
        pid: pid.clone(),
        executor_pid: None,
        output: Vec::new(),
        tx: tx
    };

    node.spawn(&pid, Box::new(process)).unwrap();

    // Wait for the process to get the timeout
    rx.recv().unwrap();

    node.shutdown();
    for h in handles {
        h.join().unwrap();
    }
}

fn run_client_operation_against_nonexistant_pid_and_wait_for_timeout(node_id: NodeId) {
    let pid = Pid {name: "fake-pid".to_string(), group: None, node: node_id};
    let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
    sock.set_nonblocking(true).unwrap();
    let mut serializer = MsgpackSerializer::new();
    send(&mut sock, &mut serializer, ApiClientMsg::Op(pid, 0));
    assert_eq!(true, wait_for(Duration::seconds(10), || {
        if let Ok(Some(ApiClientMsg::Timeout)) = serializer.read_msg(&mut sock) {
            return true;
        }
        false
    }));
}

fn shutdown(node: Node<RabbleUserMsg>,
            service_pid: Pid,
            service_tx: Sender<Envelope<RabbleUserMsg>>)
{
    // A made up pid to represent the test.
    let from = Pid {name: "test-runner".to_string(), group: None, node: node.id.clone()};
    let shutdown_envelope = Envelope {
        to: service_pid,
        from: from,
        msg: Msg::Shutdown,
        correlation_id: None
    };
    service_tx.send(shutdown_envelope).unwrap();
    node.shutdown();
}
