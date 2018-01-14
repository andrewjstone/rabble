extern crate amy;
extern crate rabble;

#[macro_use]
extern crate assert_matches;
extern crate serde;

#[macro_use]
extern crate serde_derive;

extern crate slog;
extern crate slog_stdlog;
extern crate slog_envlogger;
extern crate slog_term;
extern crate log;
extern crate time;

mod utils;

use std::{str};
use std::net::TcpStream;
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
    Envelope,
    Msg,
    Node,
    NodeId
};
use rabble::serialize::{Serialize, MsgpackSerializer};

const CLUSTER_SERVER_IP: &'static str = "127.0.0.1:11001";
const API_SERVER_IP: &'static str = "127.0.0.1:22001";

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
