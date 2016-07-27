use std::thread::{self, JoinHandle};
use amy::Sender;

use rabble::{
    Pid,
    Node,
    Envelope,
    CorrelationId,
    Msg,
    MsgpackSerializer,
    TcpServerHandler,
    Service,
    ConnectionMsg,
    ConnectionHandler
};

use super::messages::{RabbleUserMsg, ApiClientMsg};

#[allow(dead_code)] // Not used in all tests
const API_SERVER_IP: &'static str  = "127.0.0.1:12001";

#[allow(dead_code)] // Not used in all tests
pub fn start(node: Node<RabbleUserMsg>)
    -> (Pid, Sender<Envelope<RabbleUserMsg>>, JoinHandle<()>)
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
    type Msg = RabbleUserMsg;
    type ClientMsg = ApiClientMsg;

    fn new(pid: Pid, id: usize) -> ApiServerConnectionHandler {
        ApiServerConnectionHandler {
            pid: pid,
            id: id,
            total_requests: 0,
            output: Vec::with_capacity(1)
        }
    }

    fn handle_envelope(&mut self, envelope: Envelope<RabbleUserMsg>)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        let Envelope {msg, correlation_id, ..} = envelope;
        let correlation_id = correlation_id.unwrap();
        match msg {
            Msg::User(RabbleUserMsg::History(h)) => {
                self.output.push(ConnectionMsg::Client(ApiClientMsg::History(h), correlation_id));
            },
            Msg::User(RabbleUserMsg::OpComplete) => {
                self.output.push(ConnectionMsg::Client(ApiClientMsg::OpComplete, correlation_id));
            },
            Msg::Timeout => {
                self.output.push(ConnectionMsg::Client(ApiClientMsg::Timeout, correlation_id));
            },
            _ => unreachable!()
        }
        &mut self.output
    }

    fn handle_network_msg(&mut self, msg: ApiClientMsg)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        match msg {
            ApiClientMsg::Op(pid, val) => {
                self.push_new_envelope(pid, RabbleUserMsg::Op(val));
            },
            ApiClientMsg::GetHistory(pid) => {
                self.push_new_envelope(pid, RabbleUserMsg::GetHistory);
            }

            // We only handle client requests. Client replies come in as Envelopes and are handled
            // in handle_envelope().
            _ => unreachable!()
        }
        &mut self.output
    }
}

impl ApiServerConnectionHandler {
    pub fn push_new_envelope(&mut self, to: Pid, user_msg: RabbleUserMsg) {
        let msg = Msg::User(user_msg);
        let correlation_id = CorrelationId::request(self.pid.clone(), self.id, self.total_requests);
        self.total_requests += 1;
        let envelope = Envelope::new(to, self.pid.clone(), msg, Some(correlation_id));
        self.output.push(ConnectionMsg::Envelope(envelope));
    }
}
