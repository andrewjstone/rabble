use std::fmt::Debug;
use std::marker::PhantomData;
use std::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::io;
use rustc_serialize::{Encodable, Decodable};
use amy::{Registrar, Notification, Event};
use errors::*;
use handler::Handler;
use envelope::SystemEnvelope;
use node::Node;
use connection::{ConnectionTypes, Connection, MsgWriter, MsgReader, WriteResult};

/// A service handler for an async TCP server
pub struct TcpServerHandler<T, U, C>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
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
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
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
               request_timeout: usize) -> TcpServerHandler<T, U, C>
    {
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        TcpServerHandler {
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
        C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    fn init(&mut self, registrar: &Registrar, node: &Node<T, U>) -> Result<()> {
        self.listener_id = try!(registrar.register(&self.listener, Event::Read)
                                .chain_err(|| "Failed to register listener"));
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

        // TODO: check for timeouts here

        if let Some(connection) = self.connections.get_mut(&notification.id) {
            let mut writable = true;

            if notification.event.readable() {
                writable = try!(handle_readable(connection, node, registrar));
            }

            if notification.event.writable() {
                writable = try!(write_messages(connection, vec![], registrar));
            }

            return reregister(notification.id, connection, registrar, writable);
        }

        Ok(())
    }

    fn handle_system_envelope(&mut self,
                              node: &Node<T, U>,
                              envelope: SystemEnvelope<U>) -> Result<()>
    {
        Ok(())
    }
}

/// Handle any readable notifications. Returns whether the socket is still writable.
fn handle_readable<T, U, C>(connection: &mut Connection<C>,
                            node: &Node<T, U>,
                            registrar: &Registrar) -> Result<bool>
    where T: Encodable + Decodable,
          U: Debug + Clone,
          C: ConnectionTypes<Socket=TcpStream, ProcessMsg=T, SystemMsgTypeParameter=U>
{
    let mut writable = true;
    while let Some(msg) = try!(connection.msg_reader.read_msg(&mut connection.sock)) {
        let f = connection.network_msg_callback;
        let responses = f(&mut connection.state, node, msg);
        writable = try!(write_messages(connection, responses, registrar));
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
fn write_messages<C: ConnectionTypes>(
    connection: &mut Connection<C>,
    mut output: Vec<<<C as ConnectionTypes>::MsgWriter as MsgWriter>::Msg>,
    registrar: &Registrar) -> Result<bool>
{
    loop {
        match connection.msg_writer.write_msg(&mut connection.sock, output) {
            WriteResult::EmptyBuffer => {
                try!(registrar.register(&connection.sock, Event::Read)
                     .chain_err(|| "Failed to register socket for read event"));
                return Ok(true)
            },
            WriteResult::WouldBlock => {
                return Ok(false)
            },
            WriteResult::MoreMessagesInBuffer => (),
            WriteResult::Err(err) => return Err(err.into())
        }
        output = Vec::new()
    }
}

