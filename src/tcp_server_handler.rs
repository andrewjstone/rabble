use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use rustc_serialize::{Encodable, Decodable};
use amy::{Registrar, Notification, Event};
use errors::*;
use handler::{Handler, HandlerSpec};
use envelope::SystemEnvelope;
use node::Node;
use connection::{ConnectionTypes, Connection};

/// A service handler for an async TCP server
pub struct TcpServerHandler<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream>
{
    id: usize,
    default_handler: bool,
    total_connections: usize,
    listener: TcpListener,
    listener_id: usize,
    connection_timeout: Option<usize>, // ms
    request_timeout: usize, // ms
    connections: HashMap<usize, Connection<C>>,
    unused1: PhantomData<T>,
    unused2: PhantomData<U>
}

impl <T, U, C> TcpServerHandler<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream>
{
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
               default_handler: bool) -> TcpServerHandler<T, U, C> {

        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        TcpServerHandler {
            id: 0,
            default_handler: default_handler,
            total_connections: 0,
            listener: listener,
            listener_id: 0,
            connection_timeout: connection_timeout,
            request_timeout: request_timeout,
            connections: HashMap::new(),
            unused1: PhantomData,
            unused2: PhantomData
        }
    }

    fn accept_connections(&mut self, node: &Node<T, U>, registrar: &Registrar) -> Result<()> {
        loop {
            match self.listener.accept() {
                Ok((socket, _)) => {
                    try!(socket.set_nonblocking(true)
                         .chain_err(|| "Failed to make socket nonblocking"));
                    let id = try!(registrar.register(&socket, Event::Read)
                                  .chain_err(|| "Failed to register accepted socket for reading"));
                    let connection = Connection::new(id, socket);
                    self.connections.insert(id, connection);
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(());
                    }
                    return Err(e.into())
                }
            }
        }
    }
}

impl<T, U, C> Handler<T, U> for TcpServerHandler<T, U, C>
  where T: Encodable + Decodable,
        U: Debug + Clone,
        C: ConnectionTypes<Socket=TcpStream> {

    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn get_spec(&self) -> HandlerSpec {
        HandlerSpec {
            default_handler: self.default_handler,
            requires_poller: true
        }
    }

    fn register_with_poller(&mut self, registrar: &Registrar) -> Vec<usize> {
        self.listener_id = registrar.register(&self.listener, Event::Read).unwrap();
        vec![self.listener_id]
    }

    fn handle_notification(&mut self,
                           node: &Node<T, U>,
                           notification: Notification,
                           registrar: &Registrar) {
        if notification.id == self.listener_id {
            if let Err(e) = self.accept_connections(node, registrar) {
                // TODO: Log error
            }
        }

    }

    fn handle_system_envelope(&mut self, node: &Node<T, U>, envelope: SystemEnvelope<U>) {
    }
}
