use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use rustc_serialize::{Encodable, Decodable};
use amy::{Registrar, Notification, Event, Timer};
use errors::*;
use handler::Handler;
use envelope::SystemEnvelope;
use node::Node;
use connection::{ConnectionTypes, Connection, MsgWriter, MsgReader};
use timer_wheel::TimerWheel;

struct ConnectionState<C: ConnectionTypes> {
    connection: Connection<C>,
    timer_wheel_slot: usize
}

/// A service handler for an async TCP server
pub struct TcpServerHandler<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    total_connections: usize,
    listener: TcpListener,
    listener_id: usize,
    timeout: Option<usize>, // ms
    connections: HashMap<usize, ConnectionState<C>>,
    timer: Option<Timer>,
    timer_wheel: Option<TimerWheel<usize>>,
    unused1: PhantomData<T>,
    unused2: PhantomData<U>
}

impl <T, U, C> TcpServerHandler<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    /// Create a new TcpServerHandler
    ///
    /// Bind to `addr` and close a connection that hasn't received a message in `connection_timeout`
    /// ms.
    pub fn new(addr: &str,
               connection_timeout: Option<usize>) -> TcpServerHandler<T, U, C>
    {
        let timer = None;
        let mut timer_wheel = None;
        if connection_timeout.is_some() {
            timer_wheel = Some(TimerWheel::new(2));
        }
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        TcpServerHandler {
            total_connections: 0,
            listener: listener,
            listener_id: 0,
            timeout: connection_timeout,
            connections: HashMap::new(),
            timer: timer,
            timer_wheel: timer_wheel,
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
                    let connection_state = ConnectionState {
                        connection: connection,
                        timer_wheel_slot: self.timer_wheel.as_mut().map_or(0,
                                                                           |mut tw| tw.insert(id))
                    };
                    self.connections.insert(id, connection_state);
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

    fn handle_connection_notification(&mut self,
                                      notification: &Notification,
                                      node: &Node<T, U>,
                                      registrar: &Registrar) -> Result<()>
    {
        if let Some(connection_state) = self.connections.get_mut(&notification.id) {
            let mut writable = true;

            if notification.event.readable() {
                writable = try!(handle_readable(connection_state, node, registrar));
                update_connection_timeout(connection_state, &mut self.timer_wheel);
            }

            if notification.event.writable() {
                let ref mut connection = connection_state.connection;
                writable = try!(connection.msg_writer.write_msgs(&mut connection.sock, None));
            }

            try!(reregister(notification.id, &connection_state.connection, registrar, writable));
        }
        Ok(())
    }

    fn tick(&mut self, node: &Node<T, U>, registrar: &Registrar) {
        for id in self.timer_wheel.as_mut().unwrap().expire() {
            if let Some(connection_state) = self.connections.remove(&id) {
                let _ = registrar.deregister(connection_state.connection.sock);
                // TODO: Log connection timeout
            }
        }
    }
}

impl<T, U, C> Handler<T, U> for TcpServerHandler<T, U, C>
  where T: Encodable + Decodable,
        U: Debug + Clone,
        C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    fn init(&mut self, registrar: &Registrar, node: &Node<T, U>) -> Result<()> {
        self.listener_id = try!(registrar.register(&self.listener, Event::Read)
                                .chain_err(|| "Failed to register listener"));

        if self.timeout.is_some() {
            self.timer = Some(try!(registrar.set_interval(self.timeout.unwrap())
                              .chain_err(|| "Failed to register tick timer")));
        }
        Ok(())
    }

    fn handle_notification(&mut self,
                           node: &Node<T, U>,
                           notification: Notification,
                           registrar: &Registrar) -> Result<()>
    {
        if notification.id == self.listener_id {
            return self.accept_connections(node, registrar);
        }

        if self.timer.is_some() && notification.id == self.timer.as_ref().unwrap().get_id() {
            self.tick(&node, &registrar);
            return Ok(());
        }

        if let Err(e) = self.handle_connection_notification(&notification, &node, &registrar) {
            // Unwrap is correct here since the above call only fails if the connection exists
            let connection_state = self.connections.remove(&notification.id).unwrap();
            let _ = registrar.deregister(connection_state.connection.sock);
            return Err(e);
        }
        Ok (())

    }

    fn handle_system_envelope(&mut self,
                              node: &Node<T, U>,
                              envelope: SystemEnvelope<U>) -> Result<()>
    {
        Ok(())
    }
}

/// Handle any readable notifications. Returns whether the socket is still writable.
fn handle_readable<T, U, C>(connection_state: &mut ConnectionState<C>,
                            node: &Node<T, U>,
                            registrar: &Registrar) -> Result<bool>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    let mut writable = true;
    let ref mut connection = connection_state.connection;
    while let Some(msg) = try!(connection.msg_reader.read_msg(&mut connection.sock)) {
        let f = connection.network_msg_callback;
        let responses = f(&mut connection.state, node, msg);
        for r in responses {
            writable = try!(connection.msg_writer.write_msgs(&mut connection.sock, Some(r)));
        }
    }
    Ok(writable)
}

/// Reregister a socket for an existing connection
fn reregister<C: ConnectionTypes>(id: usize,
                                  connection: &Connection<C>,
                                  registrar: &Registrar,
                                  writable: bool) -> Result<()>
{
    if !writable {
        return registrar.reregister(id, &connection.sock, Event::Both)
            .chain_err(|| "Failed to register socket for read and write events");
    } else {
        return registrar.reregister(id, &connection.sock, Event::Read)
            .chain_err(|| "Failed to register socket for read events");
    }
}

fn update_connection_timeout<C>(connection_state: &mut ConnectionState<C>,
                                timer_wheel: &mut Option<TimerWheel<usize>>)
    where C: ConnectionTypes
{
    if timer_wheel.is_none() { return; }
    let mut timer_wheel = timer_wheel.as_mut().unwrap();
    timer_wheel.remove(&connection_state.connection.id, connection_state.timer_wheel_slot);
    connection_state.timer_wheel_slot = timer_wheel.insert(connection_state.connection.id);
}
