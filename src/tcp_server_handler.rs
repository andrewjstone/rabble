use std::fmt::Debug;
use std::net::{TcpListener, TcpStream};
use rustc_serialize::{Encodable, Decodable};
use handler::{Handler, HandlerSpec};
use envelope::SystemEnvelope;
use node::Node;

pub struct TcpServerHandler<T: Encodable + Decodable, U: Debug + Clone> {
    id: usize,
    total_connections: usize,
    listener: TcpListener,
    listener_id: usize
    connection_timeout: Option<usize>, // ms
    request_timeout: usize, // ms
}

impl<T, U> TcpServerHandler<T, U> where T: Encodable + Decodable, U: Debug + Clone {

    /// Create a new TcpServerHandler
    ///
    /// Bind to `addr` and close a connection that hasn't received a message in `connection_timeout`
    /// ms. Passing None for a `connection_timeout` disables connection timeouts.
    ///
    /// Timeout requests made from the handler in `request_timeout` ms. This timeout is not
    /// optional as a request can always fail or be delayed indefinitely.
    pub fn new(addr: &str,
               connection_timeout: Option<usize>,
               request_timeout: usize,
               default_handler: bool) -> TcpServerHandler<T, U> {

        let listener = TcpListener::bind(IP).unwrap();
        listener.set_nonblocking(true).unwrap();
        TcpServerHandler {
            id: 0,
            default_handler: default_handler,
            total_connections: 0,
            listener: TcpListener,
            listener_id: 0,
            connection_timeout: connection_timeout,
            request_timeout: request_timeout
        }
    }
}

impl<T, U> Handler<T, U> for TcpServerHandler<T, U>
  where T: Encodable + Decodable, U: Debug + Clone {

    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn get_spec(&self) -> HandlerSpec {
        HandlerSpec {
            default_handler: self.default_handler,
            requires_poller: true
        }
    }

    fn register_with_poller(&mut self, &Registrar) -> Vec<usize> {
        self.listener_id = registrar.register(&self.listener, Event::Read).unwrap();
        vec![self.listener_id]
    }

    fn handle_notification(&mut self,
                           node: &Node<T, U>,
                           notification: Notification,
                           registrar: &Registrar) {

    }

    fn handle_system_envelope(&mut self, node: &Node<T, U>, envelope: SystemEnvelope<U>) {
    }
}
