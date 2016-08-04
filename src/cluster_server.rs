use std::sync::mpsc::{Sender, Receiver};
use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::io::{self, Error, ErrorKind};
use net2::{TcpBuilder, TcpStreamExt};
use time::{SteadyTime, Duration};
use rustc_serialize::{Encodable, Decodable};
use msgpack::{Encoder, Decoder};
use amy::{Registrar, Notification, Event, Timer, FrameReader, FrameWriter};
use members::Members;
use node_id::NodeId;
use internal_msg::InternalMsg;
use external_msg::ExternalMsg;
use timer_wheel::TimerWheel;
use envelope::Envelope;
use orset::ORSet;

// TODO: This is totally arbitrary right now and should probably be user configurable
const MAX_FRAME_SIZE: u32 = 100*1024*1024; // 100 MB
const TICK_TIME: usize = 1000; // milliseconds
const REQUEST_TIMEOUT: usize = 5000; // milliseconds

struct Conn {
    sock: TcpStream,
    node: Option<NodeId>,
    is_client: bool,
    timer_wheel_index: usize,
    reader: FrameReader,
    writer: FrameWriter
}

impl Conn {
    pub fn new(sock: TcpStream, node: Option<NodeId>, is_client: bool) -> Conn {
        Conn {
            sock: sock,
            node: node,
            is_client: is_client,
            timer_wheel_index: 0, // Initialize with a fake value
            reader: FrameReader::new(MAX_FRAME_SIZE),
            writer: FrameWriter::new(),
        }
    }
}

/// A struct that handles cluster membership connection and routing of messages to processes on
/// other nodes.
pub struct ClusterServer<T: Encodable + Decodable> {
    node: NodeId,
    rx: Receiver<InternalMsg<T>>,
    executor_tx: Sender<Envelope<T>>,
    timer: Timer,
    timer_wheel: TimerWheel<usize>,
    listener: TcpListener,
    listener_id: usize,
    members: Members,
    connections: HashMap<usize, Conn>,
    established: HashMap<NodeId, usize>,
    registrar: Registrar
}

impl<T: Encodable + Decodable> ClusterServer<T> {
    pub fn new(node: NodeId,
               rx: Receiver<InternalMsg<T>>,
               executor_tx: Sender<Envelope<T>>,
               registrar: Registrar) -> ClusterServer<T> {
        // We don't want to actually start polling yet, so create a dummy timer.
        let dummy_timer = Timer {id: 0, fd: 0};
        let listener = TcpListener::bind(&node.addr[..]).unwrap();
        listener.set_nonblocking(true).unwrap();
        ClusterServer {
            node: node.clone(),
            rx: rx,
            executor_tx: executor_tx,
            timer: dummy_timer,
            timer_wheel: TimerWheel::new(REQUEST_TIMEOUT / TICK_TIME),
            listener: listener,
            listener_id: 0,
            members: Members::new(node),
            connections: HashMap::new(),
            established: HashMap::new(),
            registrar: registrar
        }
    }

    pub fn run(mut self) {
        self.timer = self.registrar.set_interval(TICK_TIME).unwrap();
        self.listener_id = self.registrar.register(&self.listener, Event::Read).unwrap();
        while let Ok(msg) = self.rx.recv() {
            match msg {
                InternalMsg::PollNotifications(notifications) =>
                    self.handle_poll_notifications(notifications),
                InternalMsg::Join(node) => self.join(node),
                InternalMsg::User(envelope) => self.send_remote(envelope)
            }
        }
    }

    fn send_remote(&mut self, envelope: Envelope<T>) {
        if let Some(id) = self.established.get(&envelope.to.node).cloned() {
            let mut encoded = Vec::new();
            ExternalMsg::User(envelope).encode(&mut Encoder::new(&mut encoded)).unwrap();
            if let Err(e) = self.write(id, Some(encoded)) {
                self.close(id);
            }
        }
    }

    fn handle_poll_notifications(&mut self, notifications: Vec<Notification>) {
        for n in notifications {
            match n.id {
                id if id == self.listener_id => self.accept_connection(),
                id if id == self.timer.id => self.tick(),
                _ => self.handle_socket_event(n)
            }
        }
    }

    fn handle_socket_event(&mut self, notification: Notification) {
        let id = notification.id;
        if let Err(e) = self.do_socket_io(notification) {
            self.close(id);
            // TODO: Log error
        }
    }

    fn do_socket_io(&mut self, notification: Notification) -> io::Result<()> {
        match notification.event {
            Event::Read => self.read(notification.id),
            Event::Write => self.write(notification.id, None),
            Event::Both => {
                try!(self.read(notification.id));
                self.write(notification.id, None)
            }
        }
    }

    fn read(&mut self, id: usize) -> io::Result<()> {
        let messages = try!(self.decode_messages(id));
        for msg in messages {
            match msg {
                // Members is only received as the first message on any connection
                ExternalMsg::Members{from, orset} => {
                    self.establish_connection(id, from, orset);
                    self.check_connections();
                },
                ExternalMsg::Ping => self.reset_timer(id),
                ExternalMsg::User(envelope) => self.executor_tx.send(envelope).unwrap()
            }
        }
        Ok(())
    }

    fn write(&mut self, id: usize, msg: Option<Vec<u8>>) -> io::Result<()> {
        if let Some(conn) = self.connections.get_mut(&id) {
            match conn.writer.write(&mut conn.sock, msg) {
                Ok(false) => self.registrar.reregister(id, &conn.sock, Event::Both),
                Ok(true) => Ok(()),
                Err(e) => Err(e)
            }
        } else {
            Ok(())
        }
    }

    fn reset_timer(&mut self, id: usize) {
        if let Some(conn) = self.connections.get_mut(&id) {
            self.timer_wheel.remove(&id, conn.timer_wheel_index);
            conn.timer_wheel_index = self.timer_wheel.insert(id)
        }
    }

    /// Transition a connection from unestablished to established. If there is already an
    /// established connection between these two nodes, determine which one should be closed.
    fn establish_connection(&mut self, id: usize, from: NodeId, orset: ORSet<NodeId>) {
        self.members.join(orset);
        if let Some(close_id) = self.choose_connection_to_close(id, &from) {
            self.close(close_id);
            if close_id == id {
                return;
            }
        }
        if let Some(conn) = self.connections.get_mut(&id) {
            conn.node = Some(from.clone());
            self.timer_wheel.remove(&id, conn.timer_wheel_index);
            conn.timer_wheel_index = self.timer_wheel.insert(id);
            self.established.insert(from, id);
        }
    }

    /// We only want a single connection between nodes. Choose the connection where the client side
    /// comes from a node that sorts less than the node of the server side of the connection.
    /// Return the id to remove if there is an existing connection to remove, otherwise return
    /// `None` indicating that there isn't an existing connection, so don't close the new one.
    fn choose_connection_to_close(&self, id: usize, other_node: &NodeId) -> Option<usize> {
        if let Some(other_id) = self.established.get(other_node) {
            if let Some(other_conn) = self.connections.get(&other_id) {
                // This node was the client side and sorts lower or it was the server side and sorts
                // higher. Either way, it's the connection that should be closed.
                if (other_conn.is_client && self.node < *other_node) ||
                    (!other_conn.is_client && self.node > *other_node) {
                        return Some(*other_id);
                } else {
                    return Some(id);
                }
            }
        }
        None
    }

    fn decode_messages(&mut self, id: usize) -> io::Result<Vec<ExternalMsg<T>>> {
        let mut output = Vec::new();
        if let Some(conn) = self.connections.get_mut(&id) {
           let _ = try!(conn.reader.read(&mut conn.sock));
           for frame in conn.reader.iter_mut() {
               let mut decoder = Decoder::new(&frame[..]);
               let msg = try!(Decodable::decode(&mut decoder).map_err(|e| {
                   Error::new(ErrorKind::InvalidInput,e)
               }));
               output.push(msg);
           }
        }
        Ok(output)
    }

    fn join(&mut self, node: NodeId) {
        self.members.add(node.clone());
        self.connect(node);
    }

    fn connect(&mut self, node: NodeId) {
        // TODO: Could this ever actually fail?
        let sock = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
        sock.set_nonblocking(true).unwrap();
        match sock.connect(&node.addr[..]) {
            Err(ref e) if e.kind() != io::ErrorKind::WouldBlock => {
                // TODO: Log error
            },
            _ => {
                if let Ok(id) = self.registrar.register(&sock, Event::Read) {
                    let mut conn = Conn::new(sock, Some(node), true);
                    conn.timer_wheel_index = self.timer_wheel.insert(id);
                    self.connections.insert(id, conn);
                } else {
                    // TODO: Log Error
                }
            }
        }
    }

    fn accept_connection(&mut self) {
        while let Ok((sock, _)) = self.listener.accept() {
            sock.set_nonblocking(true).unwrap();
            // The socket is writable because it was just created
            if let Ok(id) = self.registrar.register(&sock, Event::Read) {
                let mut conn = Conn::new(sock, None, false);
                let encoded = self.encode_members();
                match conn.writer.write(&mut conn.sock, Some(encoded)) {
                    Ok(false) => {
                        if let Ok(_) = self.registrar.reregister(id, &conn.sock, Event::Both) {
                            conn.timer_wheel_index = self.timer_wheel.insert(id);
                            self.connections.insert(id, conn);
                        } else {
                            // TODO: Log error
                        }
                    },
                    Ok(true) => {
                        conn.timer_wheel_index = self.timer_wheel.insert(id);
                        self.connections.insert(id, conn);
                    },
                    Err(_) => {
                        self.registrar.deregister(conn.sock);
                    }

                }
            } else {
                // TODO: Log error
            }
        }
    }

    fn tick(&mut self) {
        let expired = self.timer_wheel.expire();
        self.deregister(expired);
        self.broadcast_pings();
        self.check_connections();
    }

    fn encode_members(&mut self) -> Vec<u8> {
        let orset = self.members.get_orset();
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Members::<T> {from: self.node.clone(), orset: orset};
        msg.encode(&mut Encoder::new(&mut encoded)).unwrap();
        encoded
    }

    fn deregister(&mut self, expired: HashSet<usize>) {
        for id in expired.iter() {
            self.close(*id);
        }
    }

    /// Close an existing connection and remove all related state.
    fn close(&mut self, id: usize) {
        if let Some(conn) = self.connections.remove(&id) {
            self.registrar.deregister(conn.sock);
            self.timer_wheel.remove(&id, conn.timer_wheel_index);
            if let Some(node) = conn.node {
                self.established.remove(&node);
            }
        }
    }

    fn broadcast_pings(&mut self) {
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Ping::<T>;
        msg.encode(&mut Encoder::new(&mut encoded)).unwrap();
        for error_id in self.broadcast(encoded) {
            self.close(error_id);
        }
    }

    // Write encoded values to all connections and return the id of any connections with errors
    fn broadcast(&mut self, encoded: Vec<u8>) -> Vec<usize> {
        let mut err_ids = Vec::new();
        for (id, conn) in self.connections.iter_mut() {
            match conn.writer.write(&mut conn.sock, Some(encoded.clone())) {
                Ok(false) => {
                    if let Err(e) = self.registrar.reregister(*id, &conn.sock, Event::Both) {
                        err_ids.push(*id);
                        // TODO: Log error
                    }
                },
                Ok(true) => (),
                Err(e) => {
                    err_ids.push(*id);
                    // TODO: Log error
                }
            }
        }
        err_ids
    }

    // Ensure connections are correct based on membership state
    fn check_connections(&mut self) {
        let all = self.members.all();
        let connected: HashSet<NodeId> = self.established.keys().cloned().collect();
        let to_connect: Vec<NodeId> = all.difference(&connected)
                                       .filter(|&node| *node != self.node).cloned().collect();
        let to_disconnect: Vec<NodeId> = connected.difference(&all).cloned().collect();

        for node in to_connect {
            self.connect(node);
        }

        self.disconnect_established(to_disconnect);
    }

   fn disconnect_established(&mut self, to_disconnect: Vec<NodeId>) {
       for node in to_disconnect {
           if let Some(id) = self.established.remove(&node) {
               let conn = self.connections.remove(&id).unwrap();
               self.timer_wheel.remove(&id, conn.timer_wheel_index);
               self.registrar.deregister(conn.sock);
           }
       }
   }
}
