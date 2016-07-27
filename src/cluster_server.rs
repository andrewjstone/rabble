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

// TODO: This is totally arbitrary right now and should probably be user configurable
const MAX_FRAME_SIZE: u32 = 100*1024*1024; // 100 MB
const TICK_TIME: usize = 1000; // milliseconds

struct Conn {
    sock: TcpStream,
    addr: Option<String>,
    is_client: bool,
    reader: FrameReader,
    writer: FrameWriter,
    last_received_time: SteadyTime
}

impl Conn {
    pub fn new(sock: TcpStream, addr: Option<String>, is_client: bool) -> Conn {
        Conn {
            sock: sock,
            addr: addr,
            is_client: is_client,
            reader: FrameReader::new(MAX_FRAME_SIZE),
            writer: FrameWriter::new(),
            last_received_time: SteadyTime::now()
        }
    }
}

/// A struct that handles cluster membership connection and routing of messages to processes on
/// other nodes.
pub struct ClusterServer<T: Encodable + Decodable> {
    node: Node,
    rx: Receiver<InternalMsg<T>>,
    timer: Timer,
    listener: TcpListener,
    listener_id: usize,
    members: Members,
    unestablished: HashMap<usize, Conn>,
    established_by_id: HashMap<usize, Conn>,
    established_by_addr: HashMap<usize, Conn>,
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
            listener: listener,
            listener_id: 0,
            members: Members::new(node),
            unestablished: HashMap::new(),
            established_by_addr: HashMap::new(),
            established_by_id: HashMap::new(),
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
        let addr = node.addr.clone();
        self.members.add(node);
        self.connect(addr);
    }

    fn connect(&mut self, addr: String) {
        // TODO: Could this ever actually fail?
        let sock = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
        sock.set_nonblocking(true).unwrap();
        match sock.connect(&addr[..]) {
            Err(ref e) if e.kind() != io::ErrorKind::WouldBlock => {
                // TODO: Log error
            },
            _ => {
                if let Ok(id) = self.registrar.register(&sock, Event::Read) {
                    let conn = Conn::new(sock, Some(addr), true);
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
                            self.unestablished.insert(id, conn);
                        }
                    },
                    Ok(true) => {
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
}
