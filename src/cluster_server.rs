use std::sync::mpsc::{Sender, Receiver};
use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::io;
use net2::{TcpBuilder, TcpStreamExt};
use time::{SteadyTime, Duration};
use rustc_serialize::{Encodable, Decodable};
use msgpack::Encoder;
use amy::{Registrar, Notification, Event, Timer, FrameReader, FrameWriter};
use members::Members;
use node::Node;
use internal_msg::InternalMsg;
use external_msg::ExternalMsg;
use timer_wheel::TimerWheel;

// TODO: This is totally arbitrary right now and should probably be user configurable
const MAX_FRAME_SIZE: u32 = 100*1024*1024; // 100 MB
const TICK_TIME: usize = 1000; // milliseconds
const REQUEST_TIMEOUT: usize = 5000; // milliseconds

struct Conn {
    sock: TcpStream,
    node: Option<Node>,
    is_client: bool,
    timer_wheel_index: usize,
    reader: FrameReader,
    writer: FrameWriter
}

impl Conn {
    pub fn new(sock: TcpStream, node: Option<Node>, is_client: bool) -> Conn {
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
    node: Node,
    rx: Receiver<InternalMsg<T>>,
    timer: Timer,
    timer_wheel: TimerWheel<usize>,
    listener: TcpListener,
    listener_id: usize,
    members: Members,
    unestablished: HashMap<usize, Conn>,
    established: HashMap<usize, Conn>,
    registrar: Registrar
}

impl<T: Encodable + Decodable> ClusterServer<T> {
    pub fn new(node: Node, rx: Receiver<InternalMsg<T>>, registrar: Registrar) -> ClusterServer<T> {
        // We don't want to actually start polling yet, so create a dummy timer.
        let dummy_timer = Timer {id: 0, fd: 0};
        let listener = TcpListener::bind(&node.addr[..]).unwrap();
        listener.set_nonblocking(true).unwrap();
        ClusterServer {
            node: node.clone(),
            rx: rx,
            timer: dummy_timer,
            timer_wheel: TimerWheel::new(REQUEST_TIMEOUT / TICK_TIME),
            listener: listener,
            listener_id: 0,
            members: Members::new(node),
            unestablished: HashMap::new(),
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
                InternalMsg::User(user_msg) => self.handle_user_msg(user_msg)
            }
        }
    }

    fn handle_user_msg(&mut self, msg: T) {
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

    fn join(&mut self, node: Node) {
        self.members.add(node.clone());
        self.connect(node);
    }

    fn connect(&mut self, node: Node) {
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
                    self.unestablished.insert(id, conn);
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
                            self.unestablished.insert(id, conn);
                        }
                    },
                    Ok(true) => {
                        conn.timer_wheel_index = self.timer_wheel.insert(id);
                        self.unestablished.insert(id, conn);
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
        self.send_pings();
        self.check_connections();
    }

    fn handle_socket_event(&mut self, notification: Notification) {
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
            if let Some(conn) = self.unestablished.remove(id) {
                self.registrar.deregister(conn.sock);
                continue;
            }

            if let Some(conn) = self.established.remove(id) {
                self.registrar.deregister(conn.sock);
            }
        }
    }

    fn send_pings(&mut self) {
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Ping::<T>;
        msg.encode(&mut Encoder::new(&mut encoded)).unwrap();
        for id in self.write_encoded(encoded) {
            self.established.remove(&id);
        }
    }

    // Write encoded values to a connection and return the id of any connections with errors
    fn write_encoded(&mut self, encoded: Vec<u8>) -> Vec<usize> {
        let mut err_ids = Vec::new();
        for (id, conn) in self.established.iter_mut() {
            match conn.writer.write(&mut conn.sock, Some(encoded.clone())) {
                Ok(false) => {
                    if let Err(e) = self.registrar.reregister(*id, &conn.sock, Event::Both) {
                        self.timer_wheel.remove(id, conn.timer_wheel_index);
                        err_ids.push(*id);
                        // TODO: Log error
                    }
                },
                Ok(true) => (),
                Err(e) => {
                    self.timer_wheel.remove(id, conn.timer_wheel_index);
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
        let connected: HashSet<Node> = self.established.iter().map(|(_, conn)| {
            conn.node.as_ref().unwrap().clone()
        }).collect();
        let to_connect: Vec<Node> = all.difference(&connected)
                                       .filter(|&node| *node != self.node).cloned().collect();
        let to_disconnect: Vec<Node> = connected.difference(&all).cloned().collect();

        for node in to_connect {
            self.connect(node);
        }

        self.disconnect_established(to_disconnect);
    }

   fn disconnect_established(&mut self, to_disconnect: Vec<Node>) {
       let ids: Vec<usize> = self.established.iter().filter(|&(ref id, ref conn)| {
           to_disconnect.contains(conn.node.as_ref().unwrap())
       }).map(|(id, _)| *id).collect();

       for id in ids {
           let conn = self.established.remove(&id).unwrap();
           self.timer_wheel.remove(&id, conn.timer_wheel_index);
           self.registrar.deregister(conn.sock);
       }
   }
}
