use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use rustc_serialize::{Encodable, Decodable};
use amy::{Registrar, Notification, Event, Timer};
use errors::*;
use service_handler::ServiceHandler;
use envelope::{SystemEnvelope, Envelope};
use node;
use connection_handler::{ConnectionHandler, ConnectionMsg};
use timer_wheel::TimerWheel;
use pid::Pid;
use correlation_id::CorrelationId;
use system_msg::SystemMsg;
use executor_msg::ExecutorMsg;
use serialize::Serialize;

type Node<C: ConnectionHandler> = node::Node<C::ProcessMsg, C::SystemUserMsg>;

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
    timer_wheel_slot: usize,
    stats: ConnectionStats
}

impl<C, S> Connection<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    pub fn new(id: usize,
               conn: C,
               sock: TcpStream,
               slot: usize) -> Connection<C, S>
    {
        Connection {
            id: id,
            handler: conn,
            serializer: S::new(),
            sock: sock,
            timer_wheel_slot: slot,
            stats: ConnectionStats::new()
        }
    }
}

struct ConnectionStats {
    pub total_network_msgs_sent: usize,
    pub total_network_msgs_received: usize,
    pub total_system_envelopes_received: usize,
    pub total_system_requests_sent: usize
}

impl ConnectionStats {
    pub fn new() -> ConnectionStats {
        ConnectionStats {
            total_network_msgs_sent: 0,
            total_network_msgs_received: 0,
            total_system_envelopes_received: 0,
            total_system_requests_sent: 0
        }
    }
}

/// A service handler for an async TCP server
pub struct TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    pid: Pid,
    total_connections: usize,
    listener: TcpListener,
    listener_id: usize,
    connections: HashMap<usize, Connection<C, S>>,
    connection_timeout: Option<usize>, // ms
    connection_timer: Option<Timer>,
    connection_timer_wheel: Option<TimerWheel<usize>>,
    request_timeout: usize, // ms
    request_timer: Timer,
    request_timer_wheel: TimerWheel<CorrelationId>
}

impl <C,S> TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize

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
            total_connections: 0,
            listener: listener,
            listener_id: 0,
            connections: HashMap::new(),
            connection_timeout: connection_timeout,
            connection_timer: None,
            connection_timer_wheel: connection_timer_wheel,
            request_timeout: request_timeout,
            request_timer: Timer {id: 0, fd: 0}, // Dummy timer for now. Will be set in init()
            request_timer_wheel: TimerWheel::new(TIMER_WHEEL_SLOTS + 1)
        }
    }

    fn accept_connections(&mut self, registrar: &Registrar) -> Result<()> {
        loop {
            match self.listener.accept() {
                Ok((socket, _)) => {
                    self.new_connection(socket, registrar);
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

    /// Setup a new Connection object
    ///
    /// Make the socket nonblocking, register it for reads, and establish the connection timeout.
    fn new_connection(&mut self, sock: TcpStream, registrar: &Registrar) -> Result<()> {
        try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
        let id = try!(registrar.register(&sock, Event::Read)
                      .chain_err(|| "Failed to register new socket for reading"));
        let handler = C::new(self.pid.clone(), id);
        let slot = self.connection_timer_wheel.as_mut().map_or(0, |mut tw| tw.insert(id));
        let connection = Connection::new(id, handler, sock, slot);
        self.connections.insert(id, connection);
        Ok(())
    }

    fn handle_connection_notification(&mut self,
                                      notification: &Notification,
                                      node: &Node<C>,
                                      registrar: &Registrar) -> Result<()>
    {
        if let Some(connection) = self.connections.get_mut(&notification.id) {
            let mut writable = true;

            if notification.event.readable() {
                writable = try!(handle_readable(connection,
                                                &mut self.request_timer_wheel,
                                                node,
                                                registrar));
                update_connection_timeout(connection, &mut self.connection_timer_wheel);
            }

            if notification.event.writable() {
                writable = try!(connection.serializer.write_msgs(&mut connection.sock, None));
            }

            try!(reregister(notification.id, &connection.sock, registrar, writable));
        }
        Ok(())
    }

    fn connection_tick(&mut self, registrar: &Registrar) {
        for id in self.connection_timer_wheel.as_mut().unwrap().expire() {
            if let Some(connection) = self.connections.remove(&id) {
                let _ = registrar.deregister(connection.sock);
                // TODO: Log connection timeout
            }
        }
    }

    /// Handle request timer events and see if any requests have timed out.
    fn request_tick(&mut self,
                    node: &Node<C>,
                    registrar: &Registrar) -> Result<()>
    {
        for correlation_id in self.request_timer_wheel.expire() {
            if let Some(mut connection) = self.connections.get_mut(&correlation_id.connection) {
                let envelope = SystemEnvelope {
                    from: self.pid.clone(),
                    to: self.pid.clone(),
                    msg: SystemMsg::Timeout(correlation_id.clone()),
                    correlation_id: None
                };
                try!(run_handle_system_envelope(envelope,
                                                  correlation_id.connection,
                                                  &mut connection,
                                                  node,
                                                  &mut self.request_timer_wheel,
                                                  registrar));
            }
        }
        Ok(())
    }
}


impl<C, S> ServiceHandler<C::ProcessMsg, C::SystemUserMsg> for TcpServerHandler<C, S>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    /// Initialize the state of the handler: Register timers and tcp listen socket
    fn init(&mut self,
            registrar: &Registrar,
            node: &Node<C>) -> Result<()>
    {
        self.listener_id = try!(registrar.register(&self.listener, Event::Read)
                                .chain_err(|| "Failed to register listener"));

        let req_timeout = self.request_timeout / TIMER_WHEEL_SLOTS;
        self.request_timer = try!(registrar.set_interval(req_timeout)
                                  .chain_err(|| "Failed to register request timer"));

        if self.connection_timeout.is_some() {
            let timeout = self.connection_timeout.unwrap() / TIMER_WHEEL_SLOTS;
            self.connection_timer = Some(try!(registrar.set_interval(timeout)
                              .chain_err(|| "Failed to register connection timer")));
        }
        Ok(())
    }

    /// Handle any poll notifications
    fn handle_notification(&mut self,
                           node: &Node<C>,
                           notification: Notification,
                           registrar: &Registrar) -> Result<()>
    {
        if notification.id == self.listener_id {
            return self.accept_connections(registrar);
        }

        if notification.id == self.request_timer.get_id() {
            return self.request_tick(&node, &registrar);
        }

        if self.connection_timer.is_some()
            && notification.id == self.connection_timer.as_ref().unwrap().get_id()
        {
            self.connection_tick(&registrar);
            return Ok(());
        }

        if let Err(e) = self.handle_connection_notification(&notification, &node, &registrar) {
            // Unwrap is correct here since the above call only fails if the connection exists
            let connection = self.connections.remove(&notification.id).unwrap();
            let _ = registrar.deregister(connection.sock);
            return Err(e);
        }
        Ok (())

    }

    /// Handle a system envelope from a process or system thread
    fn handle_system_envelope(&mut self,
                              node: &Node<C>,
                              envelope: SystemEnvelope<C::SystemUserMsg>,
                              registrar: &Registrar) -> Result<()>
    {
        if envelope.correlation_id.is_none() {
            return Err(format!("No correlation id for envelope {:?}", envelope).into());
        }
        // Don't bother cancelling request timers... Just ignore the timeouts in the connection if
        // the request has already received its reply
        if let Some(mut connection) =
            self.connections.get_mut(&envelope.correlation_id.as_ref().unwrap().connection)
        {
            let correlation_id = envelope.correlation_id.as_ref().unwrap().clone();
            try!(run_handle_system_envelope(envelope,
                                              correlation_id.connection,
                                              &mut connection,
                                              node,
                                              &mut self.request_timer_wheel,
                                              registrar));
        }
        Ok(())
    }
}

/// Handle any readable notifications. Returns whether the socket is still writable.
fn handle_readable<C, S>(connection: &mut Connection<C, S>,
                      request_timer_wheel: &mut TimerWheel<CorrelationId>,
                      node: &Node<C>,
                      registrar: &Registrar) -> Result<bool>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    let mut writable = true;
    while let Some(msg) = try!(connection.serializer.read_msg(&mut connection.sock)) {
        let responses = connection.handler.handle_network_msg(msg);
        let writable = try!(handle_connection_msgs(request_timer_wheel,
                                                   responses,
                                                   connection,
                                                   node));
    }
    Ok(writable)
}

/// Reregister a socket for an existing connection
fn reregister(id: usize, sock: &TcpStream, registrar: &Registrar, writable: bool) -> Result<()>
{
    if !writable {
        return registrar.reregister(id, sock, Event::Both)
            .chain_err(|| "Failed to reregister socket for read and write events");
    } else {
        return registrar.reregister(id, sock, Event::Read)
            .chain_err(|| "Failed to reregister socket for read events");
    }
}

/// A new message has been received on a connection. Reset the timer.
fn update_connection_timeout<C, S>(connection: &mut Connection<C, S>,
                                timer_wheel: &mut Option<TimerWheel<usize>>)
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    if timer_wheel.is_none() { return; }
    let mut timer_wheel = timer_wheel.as_mut().unwrap();
    timer_wheel.remove(&connection.id, connection.timer_wheel_slot);
    connection.timer_wheel_slot = timer_wheel.insert(connection.id);
}

/// Send client replies and route envelopes
///
/// For any envelopes with correlation ids, record them in the request timer wheel.
/// Return whether the connection is writable or not
fn handle_connection_msgs<C, S>(request_timer_wheel: &mut TimerWheel<CorrelationId>,
                             msgs: Vec<ConnectionMsg<C>>,
                             connection: &mut Connection<C, S>,
                             node: &Node<C>) -> Result<(bool)>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    let mut writable = true;
    for m in msgs {
        match m {
            ConnectionMsg::Envelope(Envelope::Process(pe)) => {
                if pe.correlation_id.is_some() {
                    request_timer_wheel.insert(pe.correlation_id.as_ref().unwrap().clone());
                }
                node.send(ExecutorMsg::User(Envelope::Process(pe)));
            },
            ConnectionMsg::Envelope(Envelope::System(se))  => {
                if se.correlation_id.is_some() {
                    request_timer_wheel.insert(se.correlation_id.as_ref().unwrap().clone());
                }
                node.send(ExecutorMsg::User(Envelope::System(se)));
            },
            ConnectionMsg::ClientMsg(client_msg, correlation_id) => {
                // Respond to the client
                writable = try!(connection.serializer.write_msgs(&mut connection.sock,
                                                                 Some(&client_msg))
                                .chain_err(|| format!("Failed to write client msg: {:?}",
                                                      client_msg)));
            }
        }
    }
    Ok(writable)
}

fn run_handle_system_envelope<C, S>(envelope: SystemEnvelope<C::SystemUserMsg>,
                                   connection_id: usize,
                                   connection: &mut Connection<C, S>,
                                   node: &Node<C>,
                                   request_timer_wheel: &mut TimerWheel<CorrelationId>,
                                   registrar: &Registrar) -> Result<()>
    where C: ConnectionHandler<ClientMsg=S::Msg>,
          S: Serialize
{
    let responses = connection.handler.handle_system_envelope(envelope);
    let writable = try!(handle_connection_msgs(request_timer_wheel,
                                               responses,
                                               connection,
                                               node));
    if !writable {
        try!(registrar.reregister(connection_id, &connection.sock, Event::Both)
             .chain_err(|| "Failed to reregister socket for read and write events \
                            during run_handle_system_envelope()"));
    }
    Ok(())
}
