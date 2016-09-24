use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use rustc_serialize::{Encodable, Decodable};
use amy::{Registrar, Notification, Event, Timer};
use errors::*;
use handler::Handler;
use envelope::{SystemEnvelope, Envelope};
use node::Node;
use connection::{Connection, ConnectionMsg};
use timer_wheel::TimerWheel;
use pid::Pid;
use correlation_id::CorrelationId;
use system_msg::SystemMsg;
use executor_msg::ExecutorMsg;

// The timer wheel expirations are accurate to within 1/TIMER_WHEEL_SLOTS of the timeout
const TIMER_WHEEL_SLOTS: usize = 10;

struct ConnectionState<C: Connection> {
    id: usize,
    connection: C,
    sock: TcpStream,
    timer_wheel_slot: usize,
    stats: ConnectionStats
}

impl<C: Connection> ConnectionState<C> {
    pub fn new(id: usize, conn: C, sock: TcpStream, slot: usize) -> ConnectionState<C> {
        ConnectionState {
            id: id,
            connection: conn,
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
pub struct TcpServerHandler<C: Connection>
{
    pid: Pid,
    total_connections: usize,
    listener: TcpListener,
    listener_id: usize,
    connections: HashMap<usize, ConnectionState<C>>,
    connection_timeout: Option<usize>, // ms
    connection_timer: Option<Timer>,
    connection_timer_wheel: Option<TimerWheel<usize>>,
    request_timeout: usize, // ms
    request_timer: Timer,
    request_timer_wheel: TimerWheel<CorrelationId>
}

impl <C: Connection> TcpServerHandler<C>
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
               connection_timeout: Option<usize>) -> TcpServerHandler<C>
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
                    self.new_connection_state(socket, registrar);
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

    /// Setup a new ConnectionState object
    ///
    /// Make the socket nonblocking, register it for reads, and establish the connection timeout.
    fn new_connection_state(&mut self, sock: TcpStream, registrar: &Registrar) -> Result<()> {
        try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
        let id = try!(registrar.register(&sock, Event::Read)
                      .chain_err(|| "Failed to register new socket for reading"));
        let connection = C::new(self.pid.clone(), id);
        let slot = self.connection_timer_wheel.as_mut().map_or(0, |mut tw| tw.insert(id));
        let connection_state = ConnectionState::new(id, connection, sock, slot);
        self.connections.insert(id, connection_state);
        Ok(())
    }

    fn handle_connection_notification(&mut self,
                                      notification: &Notification,
                                      node: &Node<C::ProcessMsg, C::SystemUserMsg>,
                                      registrar: &Registrar) -> Result<()>
    {
        if let Some(conn_state) = self.connections.get_mut(&notification.id) {
            let mut writable = true;

            if notification.event.readable() {
                writable = try!(handle_readable(conn_state,
                                                &mut self.request_timer_wheel,
                                                node,
                                                registrar));
                update_connection_timeout(conn_state, &mut self.connection_timer_wheel);
            }

            if notification.event.writable() {
                let ref mut connection = conn_state.connection;
                writable = try!(connection.write_msgs(&mut conn_state.sock, None));
            }

            try!(reregister(notification.id, &conn_state.sock, registrar, writable));
        }
        Ok(())
    }

    fn connection_tick(&mut self, registrar: &Registrar) {
        for id in self.connection_timer_wheel.as_mut().unwrap().expire() {
            if let Some(connection_state) = self.connections.remove(&id) {
                let _ = registrar.deregister(connection_state.sock);
                // TODO: Log connection timeout
            }
        }
    }

    /// Handle request timer events and see if any requests have timed out.
    fn request_tick(&mut self,
                    node: &Node<C::ProcessMsg, C::SystemUserMsg>,
                    registrar: &Registrar) -> Result<()>
    {
        for correlation_id in self.request_timer_wheel.expire() {
            if let Some(mut connection_state) = self.connections.get_mut(&correlation_id.connection) {
                let envelope = SystemEnvelope {
                    from: self.pid.clone(),
                    to: self.pid.clone(),
                    msg: SystemMsg::Timeout(correlation_id.clone()),
                    correlation_id: None
                };
                try!(run_handle_system_envelope(envelope,
                                                  correlation_id.connection,
                                                  &mut connection_state,
                                                  node,
                                                  &mut self.request_timer_wheel,
                                                  registrar));
            }
        }
        Ok(())
    }
}

type Node2<C: Connection> = Node<C::ProcessMsg, C::SystemUserMsg>;

impl<C> Handler<C::ProcessMsg, C::SystemUserMsg> for TcpServerHandler<C>
  where C: Connection,
{
    /// Initialize the state of the handler: Register timers and tcp listen socket
    fn init(&mut self,
            registrar: &Registrar,
            node: &Node<C::ProcessMsg,  C::SystemUserMsg>) -> Result<()>
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
                           node: &Node2<C>,
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
            let connection_state = self.connections.remove(&notification.id).unwrap();
            let _ = registrar.deregister(connection_state.sock);
            return Err(e);
        }
        Ok (())

    }

    /// Handle a system envelope from a process or system thread
    fn handle_system_envelope(&mut self,
                              node: &Node2<C>,
                              envelope: SystemEnvelope<C::SystemUserMsg>,
                              registrar: &Registrar) -> Result<()>
    {
        if envelope.correlation_id.is_none() {
            return Err(format!("No correlation id for envelope {:?}", envelope).into());
        }
        // Don't bother cancelling request timers... Just ignore the timeouts in the connection if
        // the request has already received its reply
        if let Some(mut connection_state) =
            self.connections.get_mut(&envelope.correlation_id.as_ref().unwrap().connection)
        {
            let correlation_id = envelope.correlation_id.as_ref().unwrap().clone();
            try!(run_handle_system_envelope(envelope,
                                              correlation_id.connection,
                                              &mut connection_state,
                                              node,
                                              &mut self.request_timer_wheel,
                                              registrar));
        }
        Ok(())
    }
}

/// Handle any readable notifications. Returns whether the socket is still writable.
fn handle_readable<C>(conn_state: &mut ConnectionState<C>,
                      request_timer_wheel: &mut TimerWheel<CorrelationId>,
                      node: &Node<C::ProcessMsg, C::SystemUserMsg>,
                      registrar: &Registrar) -> Result<bool>
    where C: Connection
{
    let mut writable = true;
    while let Some(msg) = try!(conn_state.connection.read_msg(&mut conn_state.sock)) {
        let responses = conn_state.connection.handle_network_msg(msg);
        let writable = try!(handle_connection_msgs(request_timer_wheel,
                                                   responses,
                                                   conn_state,
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
fn update_connection_timeout<C>(connection_state: &mut ConnectionState<C>,
                                timer_wheel: &mut Option<TimerWheel<usize>>)
    where C: Connection
{
    if timer_wheel.is_none() { return; }
    let mut timer_wheel = timer_wheel.as_mut().unwrap();
    timer_wheel.remove(&connection_state.id, connection_state.timer_wheel_slot);
    connection_state.timer_wheel_slot = timer_wheel.insert(connection_state.id);
}

/// Send client replies and route envelopes
///
/// For any envelopes with correlation ids, record them in the request timer wheel.
/// Return whether the connection is writable or not
fn handle_connection_msgs<C>(request_timer_wheel: &mut TimerWheel<CorrelationId>,
                             msgs: Vec<ConnectionMsg<C>>,
                             conn_state: &mut ConnectionState<C>,
                             node: &Node<C::ProcessMsg, C::SystemUserMsg>)
    -> Result<(bool)> where C: Connection
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
                let ref mut connection = conn_state.connection;
                // Respond to the client
                writable = try!(connection.write_msgs(&mut conn_state.sock,
                                                                 Some(&client_msg))
                                .chain_err(|| format!("Failed to write client msg: {:?}",
                                                      client_msg)));
            }
        }
    }
    Ok(writable)
}

fn run_handle_system_envelope<C>(envelope: SystemEnvelope<C::SystemUserMsg>,
                                   connection_id: usize,
                                   conn_state: &mut ConnectionState<C>,
                                   node: &Node<C::ProcessMsg, C::SystemUserMsg>,
                                   request_timer_wheel: &mut TimerWheel<CorrelationId>,
                                   registrar: &Registrar)
    -> Result<()> where C: Connection
{
    let responses = conn_state.connection.handle_system_envelope(envelope);
    let writable = try!(handle_connection_msgs(request_timer_wheel,
                                               responses,
                                               conn_state,
                                               node));
    if !writable {
        try!(registrar.reregister(connection_id, &conn_state.sock, Event::Both)
             .chain_err(|| "Failed to reregister socket for read and write events \
                            during run_handle_system_envelope()"));
    }
    Ok(())
}
