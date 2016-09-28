extern crate amy;
extern crate rabble;
#[macro_use]
extern crate assert_matches;
extern crate rustc_serialize;

use std::{thread, time};
use std::thread::JoinHandle;
use std::net::TcpStream;
use std::str;
use amy::Sender;

use rabble::{
    Pid,
    NodeId,
    Process,
    Envelope,
    CorrelationId,
    ProcessEnvelope,
    SystemEnvelope,
    SystemMsg,
    Service,
    TcpServerHandler,
    ConnectionHandler,
    ConnectionMsg,
    MsgpackSerializer,
    Serialize,
    Node
};

// Messages sent to processes
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ProcessMsg {
    Op(usize),
    GetHistory
}

// Messages sent to system threads
#[derive(Debug, Clone)]
pub enum SystemUserMsg {
    History(Vec<usize>),
    OpComplete
}

// Messages sent over the API server TCP connections
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub enum ClientMsg {
    Op(Pid, usize),
    OpComplete,
    GetHistory(Pid),
    History(Vec<usize>)
}

/// A participant in chain replication
/// impls Process
pub struct Replica {
    pid: Pid,
    next: Option<Pid>,
    history: Vec<usize>,
    output: Vec<Envelope<ProcessMsg, SystemUserMsg>>
}

impl Replica {
    pub fn new(pid: Pid, next: Option<Pid>) -> Replica {
        Replica {
            pid: pid,
            next: next,
            history: Vec::new(),
            output: Vec::with_capacity(1)
        }
    }
}

impl Process for Replica {
    type Msg = ProcessMsg;
    type SystemUserMsg = SystemUserMsg;

    fn handle(&mut self,
              msg: ProcessMsg,
              _from: Pid,
              correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<ProcessMsg, SystemUserMsg>>
    {
        match msg {
            ProcessMsg::Op(val) => {
                let reply = Envelope::System(SystemEnvelope {
                    to: correlation_id.as_ref().unwrap().pid.clone(),
                    from: self.pid.clone(),
                    msg: SystemMsg::User(SystemUserMsg::OpComplete),
                    correlation_id: correlation_id.clone()
                });
                // If there is no next pid send the reply to the original caller in the correlation
                // id. Otherwise forward to the next process in the chain.
                let envelope = self.next.as_ref().map_or(reply, |to| {
                    Envelope::Process(ProcessEnvelope {
                        to: to.clone(),
                        from: self.pid.clone(),
                        msg: ProcessMsg::Op(val),
                        correlation_id: correlation_id
                    })
                });
                self.history.push(val);
                self.output.push(envelope);
            },
            ProcessMsg::GetHistory => {
                let envelope = Envelope::System(SystemEnvelope {
                    to: correlation_id.as_ref().unwrap().pid.clone(),
                    from: self.pid.clone(),
                    msg: SystemMsg::User(SystemUserMsg::History(self.history.clone())),
                    correlation_id: correlation_id
                });
                self.output.push(envelope);
            }
        }
        &mut self.output
    }
}

pub struct ApiServerConnectionHandler {
    pid: Pid,
    id: usize,
    total_requests: usize,
    output: Vec<ConnectionMsg<ApiServerConnectionHandler>>
}

impl ConnectionHandler for ApiServerConnectionHandler {
    type ProcessMsg = ProcessMsg;
    type SystemUserMsg = SystemUserMsg;
    type ClientMsg = ClientMsg;

    fn new(pid: Pid, id: usize) -> ApiServerConnectionHandler {
        ApiServerConnectionHandler {
            pid: pid,
            id: id,
            total_requests: 0,
            output: Vec::with_capacity(1)
        }
    }

    fn handle_system_envelope(&mut self, envelope: SystemEnvelope<SystemUserMsg>)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        let SystemEnvelope {msg, correlation_id, ..} = envelope;
        let correlation_id = correlation_id.unwrap();
        match msg {
            SystemMsg::User(SystemUserMsg::History(h)) => {
                self.output.push(ConnectionMsg::Client(ClientMsg::History(h), correlation_id));
            },
            SystemMsg::User(SystemUserMsg::OpComplete) => {
                self.output.push(ConnectionMsg::Client(ClientMsg::OpComplete, correlation_id));
            },
            _ => ()
        }
        &mut self.output
    }

    fn handle_network_msg(&mut self, msg: ClientMsg)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        match msg {
            ClientMsg::Op(pid, val) => {
                self.push_new_process_envelope(pid, ProcessMsg::Op(val));
            },
            ClientMsg::GetHistory(pid) => {
                self.push_new_process_envelope(pid, ProcessMsg::GetHistory);
            }

            // We only handle client requests. Client replies come in as SystemEnvelopes
            _ => unreachable!()
        }
        &mut self.output
    }
}

impl ApiServerConnectionHandler {
    pub fn push_new_process_envelope(&mut self, to: Pid, msg: ProcessMsg) {
        let correlation_id = CorrelationId::request(self.pid.clone(), self.id, self.total_requests);
        self.total_requests += 1;
        let envelope = Envelope::Process(ProcessEnvelope {
            to: to,
            from: self.pid.clone(),
            msg: msg,
            correlation_id: Some(correlation_id)
        });
        self.output.push(ConnectionMsg::Envelope(envelope));
    }
}

const API_SERVER_IP: &'static str  = "127.0.0.1:12001";

#[test]
fn chain_replication() {
    let node_id = NodeId {name: "node1".to_string(), addr: "127.0.0.1:11001".to_string()};
    let test_pid = Pid { name: "test-runner".to_string(), group: None, node: node_id.clone()};
    let (node, mut handles) = rabble::rouse::<ProcessMsg, SystemUserMsg>(node_id);

    let pids = create_replica_pids(&node.id);

    let (service_pid, service_tx, service_handle) = start_tcp_server_service(node.clone());
    handles.push(service_handle);

    spawn_replicas(&node, &pids);

    run_client_operations(&pids);

    verify_histories(&pids);

    shutdown(node, test_pid, service_pid, service_tx);

    for h in handles {
        h.join().unwrap();
    }

}

fn shutdown(node: Node<ProcessMsg, SystemUserMsg>,
            test_pid: Pid,
            service_pid: Pid,
            service_tx: Sender<SystemEnvelope<SystemUserMsg>>)
{
    let shutdown_envelope = SystemEnvelope {
        to: service_pid,
        from: test_pid,
        msg: SystemMsg::Shutdown,
        correlation_id: None
    };
    service_tx.send(shutdown_envelope).unwrap();
    node.shutdown();

}

fn start_tcp_server_service(node: Node<ProcessMsg, SystemUserMsg>)
-> (Pid, Sender<SystemEnvelope<SystemUserMsg>>, JoinHandle<()>)
{
    let server_pid = Pid {
        name: "api-server".to_string(),
        group: None,
        node: node.id.clone()
    };

    // Start the API tcp server
    let handler: TcpServerHandler<ApiServerConnectionHandler, MsgpackSerializer<ClientMsg>> =
        TcpServerHandler::new(server_pid.clone(), API_SERVER_IP, 5000, None);
    let mut service = Service::new(server_pid, node, handler).unwrap();
    let service_tx = service.tx.clone();
    let service_pid = service.pid.clone();
    let h = thread::spawn(move || {
        service.wait();
    });
    (service_pid, service_tx, h)
}


fn create_replica_pids(node_id: &NodeId) -> Vec<Pid> {
    ["replica1", "replica2", "replica3"].iter().map(|name| {
        Pid {
            name: name.to_string(),
            group: None,
            node: node_id.clone()
        }
    }).collect()
}

fn spawn_replicas(node: &Node<ProcessMsg, SystemUserMsg>, pids: &Vec<Pid>) {
    // Launch the three replicas participating in chain replication
    for i in 0..pids.len() {
        let next = if i == pids.len() - 1 {
            None
        } else {
            Some(pids[i + 1].clone())
        };

        let replica = Box::new(Replica::new(pids[i].clone(), next));
        node.spawn(pids[i].clone(), replica).unwrap();
    }
}

/// launch 3 clients and send concurrent operations to the head of the chain
fn run_client_operations(pids: &Vec<Pid>) {
    let mut client_handles = Vec::new();
    for i in 0..3 {
        let pids = pids.clone();
        let h = thread::spawn(move || {
            let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
            let mut serializer = MsgpackSerializer::new();
            assert_matches!(serializer.write_msgs(&mut sock,
                                                  Some(&ClientMsg::Op(pids[0].clone(), i))),
                            Ok(true));
            sock.set_nonblocking(true).unwrap();
            loop {
                thread::sleep(time::Duration::from_millis(10));
                match serializer.read_msg(&mut sock) {
                    Ok(None) => (),
                    Ok(Some(reply)) => {
                        assert_eq!(ClientMsg::OpComplete, reply);
                        break;
                    },
                    Err(e) => {
                        println!("{}", e);
                        assert!(false)
                    }
                }
            }
        });
        client_handles.push(h);
    }

    for h in client_handles {
        h.join().unwrap();
    }
}

/// Verify that after all client operations have gotten replies that the history of operations in
/// each replica is identical.
fn verify_histories(pids: &Vec<Pid>) {
    let pids = pids.clone();
    let h = thread::spawn(move || {
        let mut sock = TcpStream::connect(API_SERVER_IP).unwrap();
        sock.set_nonblocking(true).unwrap();
        let mut serializer = MsgpackSerializer::new();
        let mut history = Vec::new();
        for pid in pids {
            assert_matches!(serializer.write_msgs(&mut sock,
                                                  Some(&ClientMsg::GetHistory(pid))),
                                                  Ok(true));
            loop {
                thread::sleep(time::Duration::from_millis(10));
                match serializer.read_msg(&mut sock) {
                    Ok(None) => (),
                    Ok(Some(ClientMsg::History(h))) => {
                        if history.len() == 0 {
                            history = h;
                        } else {
                            assert_eq!(history, h);
                            assert!(history.len() != 0);
                        }
                        break;
                    },
                    Ok(val) => {
                        println!("{:?}", val);
                        assert!(false)
                    },
                    Err(e) => {
                        println!("{}", e);
                        assert!(false)
                    }
                }
            }
        }
    });
    h.join().unwrap();
}
