use std::thread::{self, JoinHandle};
use amy::Sender;

use rabble::{
    Pid,
    Node,
    Envelope,
    SystemEnvelope,
    ProcessEnvelope,
    CorrelationId,
    SystemMsg,
    MsgpackSerializer,
    TcpServerHandler,
    Service,
    ConnectionMsg,
    ConnectionHandler
};

use super::messages::{ProcessMsg, SystemUserMsg, ApiClientMsg};

const API_SERVER_IP: &'static str  = "127.0.0.1:12001";

pub fn start(node: Node<ProcessMsg, SystemUserMsg>)
    -> (Pid, Sender<SystemEnvelope<SystemUserMsg>>, JoinHandle<()>)
{
    let server_pid = Pid {
        name: "api-server".to_string(),
        group: None,
        node: node.id.clone()
    };

    // Start the API tcp server
    let handler: TcpServerHandler<ApiServerConnectionHandler, MsgpackSerializer<ApiClientMsg>> =
        TcpServerHandler::new(server_pid.clone(), API_SERVER_IP, 5000, None);
    let mut service = Service::new(server_pid, node, handler).unwrap();
    let service_tx = service.tx.clone();
    let service_pid = service.pid.clone();
    let h = thread::spawn(move || {
        service.wait();
    });
    (service_pid, service_tx, h)
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
    type ClientMsg = ApiClientMsg;

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
                self.output.push(ConnectionMsg::Client(ApiClientMsg::History(h), correlation_id));
            },
            SystemMsg::User(SystemUserMsg::OpComplete) => {
                self.output.push(ConnectionMsg::Client(ApiClientMsg::OpComplete, correlation_id));
            },
            _ => ()
        }
        &mut self.output
    }

    fn handle_network_msg(&mut self, msg: ApiClientMsg)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        match msg {
            ApiClientMsg::Op(pid, val) => {
                self.push_new_process_envelope(pid, ProcessMsg::Op(val));
            },
            ApiClientMsg::GetHistory(pid) => {
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
