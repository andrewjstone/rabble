use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use std::fmt::Debug;
use serde;
use amy::{Registrar, Notification, Event};
use errors::*;
use msg::Msg;
use envelope::Envelope;
use node::Node;
use timer_wheel::TimerWheel;
use pid::Pid;
use correlation_id::CorrelationId;
use serialize::Serialize;
use super::{ServiceHandler, ConnectionHandler, ConnectionMsg};

// The timer wheel expirations are accurate to within 1/TIMER_WHEEL_SLOTS of the timeout
const TIMER_WHEEL_SLOTS: usize = 10;

struct Connection<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    id: usize,
    handler: C,
    serializer: S,
    sock: TcpStream,
    timer_wheel_slot: usize
}

impl<C, S> Connection<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    pub fn new(id: usize,
               handler: C,
               sock: TcpStream,
               slot: usize) -> Connection<C, S>
    {
        Connection {
            id: id,
            handler: handler,
            serializer: S::new(),
            sock: sock,
            timer_wheel_slot: slot
        }
    }
}

/// A service handler for an async TCP server
pub struct TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    pid: Pid,
    listener: TcpListener,
    listener_id: usize,
    connections: HashMap<usize, Connection<C, S>>,
    connection_timeout: Option<usize>, // ms
    connection_timer_id: Option<usize>,
    connection_timer_wheel: Option<TimerWheel<usize>>,
    request_timeout: usize, // ms
    request_timer_id: usize,
    request_timer_wheel: TimerWheel<CorrelationId>,
    output: Vec<ConnectionMsg<C>>

}

impl<'de, C, S> TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize,
          C::Msg: serde::Serialize + serde::Deserialize<'de> + Clone + Debug

{
    /// Create a new TcpServerHandler
    ///
    /// Bind to `addr` and close a connection that hasn't received a message in `connection_timeout`
    /// ms. Note that the connection timeout is optional.
    ///
    /// Every request with a CorrelationId is also tracked with a timer. This `request_timeout` is
    /// not optional as every request can potentially fail, or be delayed indefinitely.
    pub fn new(pid: Pid,
               addr: &str,
               request_timeout: usize,
               connection_timeout: Option<usize>) -> TcpServerHandler<C, S>
    {
        let mut connection_timer_wheel = None;
        if connection_timeout.is_some() {
            connection_timer_wheel = Some(TimerWheel::new(TIMER_WHEEL_SLOTS + 1));
        }
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        TcpServerHandler {
            pid: pid,
            listener: listener,
            listener_id: 0,
            connections: HashMap::new(),
            connection_timeout: connection_timeout,
            connection_timer_id: None,
            connection_timer_wheel: connection_timer_wheel,
            request_timeout: request_timeout,
            request_timer_id: 0, // Dummy timer id for now. Will be set in init()
            request_timer_wheel: TimerWheel::new(TIMER_WHEEL_SLOTS + 1),
            output: Vec::new()
        }
    }

    fn accept_connections(&mut self, registrar: &Registrar) -> Result<()> {
        loop {
            match self.listener.accept() {
                Ok((socket, _)) => {
                    try!(self.new_connection(socket, registrar));
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(())
                    }
                    return Err(e.into())
                }
            }
        }
    }

    /// Setup a new Connection object
    ///
    /// Make the socket nonblocking, register it for reads, and establish the connection timeout.
    fn new_connection(&mut self, sock: TcpStream, registrar: &Registrar) -> Result<()> {
        try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
        let id = try!(registrar.register(&sock, Event::Read)
                      .chain_err(|| "Failed to register new socket for reading"));
        let handler = C::new(self.pid.clone(), id as u64);
        let slot = self.connection_timer_wheel.as_mut().map_or(0, |tw| tw.insert(id));
        let connection = Connection::new(id, handler, sock, slot);
        self.connections.insert(id, connection);
        Ok(())
    }

    fn handle_connection_notification(&mut self,
                                      notification: &Notification,
                                      node: &Node<C::Msg>) -> Result<()>
    {
        if let Some(connection) = self.connections.get_mut(&notification.id) {
            if notification.event.writable() {
                // Notify the serializer that the socket is writable again
                connection.serializer.set_writable();
                try!(connection.serializer.write_msgs(&mut connection.sock, None));
            }

            if notification.event.readable() {
                try!(handle_readable(connection,
                                     &mut self.request_timer_wheel,
                                     node,
                                     &mut self.output));
                update_connection_timeout(connection, &mut self.connection_timer_wheel);
            }
        }
        Ok(())
    }

    fn connection_tick(&mut self, registrar: &Registrar) {
        for id in self.connection_timer_wheel.as_mut().unwrap().expire() {
            if let Some(connection) = self.connections.remove(&id) {
                let _ = registrar.deregister(&connection.sock);
                // TODO: Log connection timeout
            }
        }
    }

    /// Handle request timer events and see if any requests have timed out.
    fn request_tick(&mut self, node: &Node<C::Msg>) -> Result<()>{
        for correlation_id in self.request_timer_wheel.expire() {
            let conn_id = correlation_id.connection.as_ref().unwrap();
            if let Some(connection) = self.connections.get_mut(&(*conn_id as usize)) {
                let envelope = Envelope {
                    from: self.pid.clone(),
                    to: self.pid.clone(),
                    msg: Msg::Timeout,
                    correlation_id: Some(correlation_id.clone())
                };
                connection.handler.handle_envelope(envelope, &mut self.output);
                try!(handle_connection_msgs(&mut self.request_timer_wheel,
                                            &mut self.output,
                                            &mut connection.serializer,
                                            &mut connection.sock,
                                            node));
            }
        }
        Ok(())
    }
}


impl<'de, C, S> ServiceHandler<C::Msg> for TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize,
          C::Msg: serde::Serialize + serde::Deserialize<'de> + Clone + Debug
{
    /// Initialize the state of the handler: Register timers and tcp listen socket
    fn init(&mut self,
            registrar: &Registrar,
            _node: &Node<C::Msg>) -> Result<()>
    {
        self.listener_id = try!(registrar.register(&self.listener, Event::Read)
                                .chain_err(|| "Failed to register listener"));

        let req_timeout = self.request_timeout / TIMER_WHEEL_SLOTS;
        self.request_timer_id = try!(registrar.set_interval(req_timeout)
                                  .chain_err(|| "Failed to register request timer"));

        if self.connection_timeout.is_some() {
            let timeout = self.connection_timeout.unwrap() / TIMER_WHEEL_SLOTS;
            self.connection_timer_id = Some(try!(registrar.set_interval(timeout)
                              .chain_err(|| "Failed to register connection timer")));
        }
        Ok(())
    }

    /// Handle any poll notifications
    fn handle_notification(&mut self,
                           node: &Node<C::Msg>,
                           notification: Notification,
                           registrar: &Registrar) -> Result<()>
    {
        if notification.id == self.listener_id {
            return self.accept_connections(registrar);
        }

        if notification.id == self.request_timer_id {
            return self.request_tick(&node);
        }

        if self.connection_timer_id.is_some()
            && notification.id == self.connection_timer_id.unwrap()
        {
            self.connection_tick(&registrar);
            return Ok(());
        }

        if let Err(e) = self.handle_connection_notification(&notification, &node) {
            // Unwrap is correct here since the above call only fails if the connection exists
            let connection = self.connections.remove(&notification.id).unwrap();
            let _ = registrar.deregister(&connection.sock);
            let errmsg = e.to_string();
            return Err(e).chain_err(|| format!("{}: id {}", errmsg, notification.id));
        }
        Ok (())

    }

    /// Handle an envelope from a process or service
    fn handle_envelope(&mut self,
                       node: &Node<C::Msg>,
                       envelope: Envelope<C::Msg>,
                       _registrar: &Registrar) -> Result<()>
    {
        if envelope.correlation_id.is_none() {
            return Err(format!("No correlation id for envelope {:?}", envelope).into());
        }
        // Don't bother cancelling request timers... Just ignore the timeouts in the connection if
        // the request has already received its reply
        let conn_id = envelope.correlation_id.as_ref().unwrap().connection.as_ref().cloned().unwrap();
        if let Some(connection) = self.connections.get_mut(&(conn_id as usize)) {
            connection.handler.handle_envelope(envelope, &mut self.output);
            try!(handle_connection_msgs(&mut self.request_timer_wheel,
                                        &mut self.output,
                                        &mut connection.serializer,
                                        &mut connection.sock,
                                        node));

        }
        Ok(())
    }
}

/// Handle any readable notifications.
fn handle_readable<'de, C, S>(connection: &mut Connection<C, S>,
                      request_timer_wheel: &mut TimerWheel<CorrelationId>,
                      node: &Node<C::Msg>,
                      output: &mut Vec<ConnectionMsg<C>>) -> Result<()>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize,
          C::Msg: serde::Serialize + serde::Deserialize<'de> + Clone + Debug
{
    while let Some(msg) = try!(connection.serializer.read_msg(&mut connection.sock)) {
        connection.handler.handle_network_msg(msg, output);
        try!(handle_connection_msgs(request_timer_wheel,
                                    output,
                                    &mut connection.serializer,
                                    &mut connection.sock,
                                    node));
    }
    Ok(())
}

/// A new message has been received on a connection. Reset the timer.
fn update_connection_timeout<C, S>(connection: &mut Connection<C, S>,
                                timer_wheel: &mut Option<TimerWheel<usize>>)
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    if timer_wheel.is_none() { return; }
    let timer_wheel = timer_wheel.as_mut().unwrap();
    timer_wheel.remove(&connection.id, connection.timer_wheel_slot);
    connection.timer_wheel_slot = timer_wheel.insert(connection.id);
}

/// Send client replies and route envelopes
///
/// For any envelopes with correlation ids, record them in the request timer wheel.
fn handle_connection_msgs<'de, C, S>(request_timer_wheel: &mut TimerWheel<CorrelationId>,
                             msgs: &mut Vec<ConnectionMsg<C>>,
                             serializer: &mut S,
                             sock: &mut TcpStream,
                             node: &Node<C::Msg>) -> Result<()>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize,
          C::Msg: serde::Serialize + serde::Deserialize<'de> + Clone + Debug
{
    for m in msgs.drain(..) {
        match m {
            ConnectionMsg::Envelope(envelope) => {
                if envelope.correlation_id.is_some() {
                    request_timer_wheel.insert(envelope.correlation_id.as_ref().unwrap().clone());
                }
                node.send(envelope).unwrap();
            },
            ConnectionMsg::Client(client_msg, _) => {
                // Respond to the client
                try!(serializer.write_msgs(sock, Some(&client_msg))
                     .chain_err(|| format!("Failed to write client msg: {:?}",
                                           client_msg)));
            }
        }
    }
    Ok(())
}
