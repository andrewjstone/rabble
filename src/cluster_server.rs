use std::sync::mpsc::{self, Sender, Receiver};
use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::fmt::Debug;
use std;
use libc::EINPROGRESS;
use net2::{TcpBuilder, TcpStreamExt};
use rustc_serialize::{Encodable, Decodable};
use msgpack::{Encoder, Decoder};
use slog;
use amy::{Registrar, Notification, Event, Timer, FrameReader, FrameWriter};
use members::Members;
use node_id::NodeId;
use cluster_msg::ClusterMsg;
use executor_msg::ExecutorMsg;
use external_msg::ExternalMsg;
use timer_wheel::TimerWheel;
use envelope::{Envelope, ProcessEnvelope, SystemEnvelope};
use orset::ORSet;
use pid::Pid;
use system_msg::SystemMsg;
use cluster_status::ClusterStatus;
use correlation_id::CorrelationId;
use errors::*;

// TODO: This is totally arbitrary right now and should probably be user configurable
const MAX_FRAME_SIZE: u32 = 100*1024*1024; // 100 MB
const TICK_TIME: usize = 1000; // milliseconds
const REQUEST_TIMEOUT: usize = 5000; // milliseconds

struct Conn {
    sock: TcpStream,
    node: Option<NodeId>,
    is_client: bool,
    members_sent: bool,
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
            members_sent: false,
            timer_wheel_index: 0, // Initialize with a fake value
            reader: FrameReader::new(MAX_FRAME_SIZE),
            writer: FrameWriter::new(),
        }
    }
}

/// A struct that handles cluster membership connection and routing of messages to processes on
/// other nodes.
pub struct ClusterServer<T: Encodable + Decodable, U: Debug> {
    pid: Pid,
    node: NodeId,
    rx: Receiver<ClusterMsg<T>>,
    executor_tx: Sender<ExecutorMsg<T, U>>,
    timer: Timer,
    timer_wheel: TimerWheel<usize>,
    listener: TcpListener,
    listener_id: usize,
    members: Members,
    connections: HashMap<usize, Conn>,
    established: HashMap<NodeId, usize>,
    registrar: Registrar,
    logger: slog::Logger
}

impl<T: Encodable + Decodable, U: Debug> ClusterServer<T, U> {
    pub fn new(node: NodeId,
               rx: Receiver<ClusterMsg<T>>,
               executor_tx: Sender<ExecutorMsg<T, U>>,
               registrar: Registrar,
               logger: slog::Logger) -> ClusterServer<T, U> {
        let pid = Pid {
            group: Some("rabble".to_string()),
            name: "ClusterServer".to_string(),
            node: node.clone()
        };
        // We don't want to actually start polling yet, so create a dummy timer.
        let dummy_timer = Timer {id: 0, fd: 0};
        let listener = TcpListener::bind(&node.addr[..]).unwrap();
        listener.set_nonblocking(true).unwrap();
        ClusterServer {
            pid: pid,
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
            registrar: registrar,
            logger: logger.new(o!("component" => "cluster_server"))
        }
    }

    pub fn run(mut self) {
        info!(self.logger, "Starting");
        self.timer = self.registrar.set_interval(TICK_TIME).unwrap();
        self.listener_id = self.registrar.register(&self.listener, Event::Read).unwrap();
        while let Ok(msg) = self.rx.recv() {
            if let Err(e) = self.handle_cluster_msg(msg) {
                for id in e.kind().get_ids() {
                    self.close(id)
                }
                match *e.kind() {
                    ErrorKind::EncodeError(..) | ErrorKind::DecodeError(..) |
                    ErrorKind::RegistrarError(..) | ErrorKind::SendError(..) =>
                        error!(self.logger, e.to_string()),

                    ErrorKind::Shutdown(..) => {
                        info!(self.logger, e.to_string());
                        break;
                    },

                    _ => warn!(self.logger, e.to_string())
                }
            }
        }
    }

    fn handle_cluster_msg(&mut self, msg: ClusterMsg<T>) -> Result<()> {
        match msg {
            ClusterMsg::PollNotifications(notifications) =>
                self.handle_poll_notifications(notifications),
            ClusterMsg::Join(node) => self.join(node),
            ClusterMsg::User(envelope) => self.send_remote(envelope),
            ClusterMsg::GetStatus(correlation_id) => self.get_status(correlation_id),
            ClusterMsg::Shutdown => Err(ErrorKind::Shutdown(self.pid.clone()).into())
        }
    }

    fn get_status(&self, correlation_id: CorrelationId) -> Result<()> {
        let status = ClusterStatus {
            members: self.members.clone(),
            connected: self.established.keys().cloned().collect()
        };
        let system_envelope = SystemEnvelope {
            to: correlation_id.pid.clone(),
            from: self.pid.clone(),
            msg: SystemMsg::ClusterStatus(status),
            correlation_id: Some(correlation_id)
        };
        // Route the response through the executor since it knows how to contact all Pids
        let envelope = Envelope::System(system_envelope);
        if let Err(mpsc::SendError(ExecutorMsg::User(Envelope::System(se)))) =
            self.executor_tx.send(ExecutorMsg::User(envelope))
        {
            return Err(ErrorKind::SendError("ExecutorMsg::User".to_string(), Some(se.to)).into());
        }
        Ok(())
    }

    fn send_remote(&mut self, envelope: ProcessEnvelope<T>) -> Result<()> {
        if let Some(id) = self.established.get(&envelope.to.node).cloned() {
            trace!(self.logger, "send remote"; "to" => envelope.to.to_string());
            let mut encoded = Vec::new();
            let node = envelope.to.node.clone();
            try!(ExternalMsg::User(envelope).encode(&mut Encoder::new(&mut encoded))
                .chain_err(|| ErrorKind::EncodeError(Some(id), Some(node))));
            try!(self.write(id, Some(encoded)));
        }
        Ok(())
    }

    fn handle_poll_notifications(&mut self, notifications: Vec<Notification>) -> Result<()> {
        let mut errors = Vec::new();
        for n in notifications {
            let result = match n.id {
                id if id == self.listener_id => self.accept_connection(),
                id if id == self.timer.id => self.tick(),
                _ => self.do_socket_io(n)
            };

            if let Err(e) = result {
                errors.push(e);
            }
        }
        if errors.len() != 0 {
            return Err(ErrorKind::PollNotificationErrors(errors).into());
        }
        Ok(())
    }

    fn do_socket_io(&mut self, notification: Notification) -> Result<()> {
        let id = notification.id;
        match notification.event {
            Event::Read => self.read(notification.id),
            Event::Write => self.write(notification.id, None),
            Event::Both => {
                try!(self.read(notification.id));
                self.write(notification.id, None)
            }
        }
    }

    /// Returns `Some(true)` if there is such a connection and the members were already sent.
    /// Returns `Some(false)` if there is such a connection and the members were NOT sent.
    /// Returns None if there is no such connection.
    fn members_sent(&self, id: usize) -> Option<bool> {
        if let Some(conn) = self.connections.get(&id) {
            return Some(conn.members_sent);
        }
        None
    }

    fn read(&mut self, id: usize) -> Result<()> {
        match self.members_sent(id) {
            Some(false) => try!(self.send_members(id)),
            None => (),
            Some(true) => {
                let messages = try!(self.decode_messages(id));
                for msg in messages {
                    try!(self.handle_decoded_message(id, msg));
                }
            }
        }
        Ok(())
    }

    fn handle_decoded_message(&mut self, id: usize, msg: ExternalMsg<T>) -> Result<()> {
        match msg {
            ExternalMsg::Members{from, orset} => {
                info!(self.logger, "Got Members"; "from" => from.to_string());
                self.establish_connection(id, from, orset);
                self.check_connections();
            },
            ExternalMsg::Ping => {
                trace!(self.logger, "Got Ping"; "id" => id);
                self.reset_timer(id);
            }
            ExternalMsg::User(process_envelope) => {
                debug!(self.logger, "Got User Message";
                       "from" => process_envelope.from.to_string(),
                       "to" => process_envelope.to.to_string());
                let envelope = Envelope::Process(process_envelope);
                if let Err(mpsc::SendError(ExecutorMsg::User(Envelope::Process(pe))))
                    = self.executor_tx.send(ExecutorMsg::User(envelope))
                {
                    return Err(ErrorKind::SendError("ExecutorMsg::User".to_string(),
                                                    Some(pe.to)).into());
                }
            }
        }
        Ok(())
    }

    fn write(&mut self, id: usize, msg: Option<Vec<u8>>) -> Result<()> {
        if let Some(conn) = self.connections.get_mut(&id) {
            let writable = try!(conn.writer.write(&mut conn.sock, msg)
                                .chain_err(|| ErrorKind::WriteError(id, conn.node.clone())));
            if !writable {
                try!(self.registrar.reregister(id, &conn.sock, Event::Both)
                    .chain_err(|| ErrorKind::RegistrarError(Some(id), conn.node.clone())));
            }
        }
        Ok(())
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
        info!(self.logger, "Establish connection"; "peer" => from.to_string());
        self.members.join(orset);
        if let Some(close_id) = self.choose_connection_to_close(id, &from) {
            debug!(self.logger,
                   "Two connections between nodes. Closing the connection where \
                    the peer that sorts lower was the connecting client";
                    "peer" => from.to_string());
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

    fn decode_messages(&mut self, id: usize) -> Result<Vec<ExternalMsg<T>>> {
        let mut output = Vec::new();
        if let Some(conn) = self.connections.get_mut(&id) {
            let node = conn.node.clone();
            try!(conn.reader.read(&mut conn.sock)
                 .chain_err(|| ErrorKind::ReadError(id, node.clone())));
            try!(self.registrar.reregister(id, &conn.sock, Event::Read)
                 .chain_err(|| ErrorKind::RegistrarError(Some(id), node.clone())));

            for frame in conn.reader.iter_mut() {
                let mut decoder = Decoder::new(&frame[..]);
                let msg = try!(Decodable::decode(&mut decoder)
                               .chain_err(|| ErrorKind::DecodeError(id, node.clone())));
                output.push(msg);
            }
        }
        Ok(output)
    }

    fn join(&mut self, node: NodeId) -> Result<()> {
        self.members.add(node.clone());
        self.connect(node)
    }

    fn connect(&mut self, node: NodeId) -> Result<()> {
        let sock = try!(TcpBuilder::new_v4().chain_err(|| "Failed to create a IPv4 socket"));
        let sock = try!(sock.to_tcp_stream().chain_err(|| "Failed to create TcpStream"));
        try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
        if let Err(e) = sock.connect(&node.addr[..]) {
            if e.raw_os_error().is_some() && *e.raw_os_error().as_ref().unwrap() != EINPROGRESS {
                return Err(e).chain_err(|| ErrorKind::ConnectError(node));
            }
        }
        try!(self.init_connection(sock));
        Ok(())
    }

    fn accept_connection(&mut self) -> Result<()> {
        while let Ok((sock, _)) = self.listener.accept() {
            try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
            let id = try!(self.init_connection(sock));
            try!(self.send_members(id));
        }
        Ok(())
    }

    fn init_connection(&mut self, sock: TcpStream) -> Result<usize> {
        let id = try!(self.registrar.register(&sock, Event::Read)
                      .chain_err(|| ErrorKind::RegistrarError(None, None)));
        let mut conn = Conn::new(sock, None, false);
        conn.timer_wheel_index = self.timer_wheel.insert(id);
        self.connections.insert(id, conn);
        Ok(id)
    }

    fn send_members(&mut self, id: usize) -> Result<()> {
        let encoded = try!(self.encode_members(id));
        if let Some(conn) = self.connections.get_mut(&id) {
            let writable = try!(conn.writer.write(&mut conn.sock, Some(encoded))
                                .chain_err(|| ErrorKind::WriteError(id, None)));
            if !writable {
                try!(self.registrar.reregister(id, &conn.sock, Event::Both)
                     .chain_err(|| ErrorKind::RegistrarError(Some(id), None)));
            }
            conn.members_sent = true;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        let expired = self.timer_wheel.expire();
        self.deregister(expired);
        try!(self.broadcast_pings());
        self.check_connections();
        Ok(())
    }

    fn encode_members(&self, id: usize) -> Result<Vec<u8>> {
        let orset = self.members.get_orset();
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Members::<T> {from: self.node.clone(), orset: orset};
        try!(msg.encode(&mut Encoder::new(&mut encoded))
             .chain_err(|| ErrorKind::EncodeError(Some(id), None)));
        Ok(encoded)
    }

    fn deregister(&mut self, expired: HashSet<usize>) {
        for id in expired.iter() {
            warn!(self.logger, "Connection timeout"; "id" => *id);
            self.close(*id);
        }
    }

    /// Close an existing connection and remove all related state.
    fn close(&mut self, id: usize) {
        if let Some(conn) = self.connections.remove(&id) {
            let _ = self.registrar.deregister(conn.sock);
            self.timer_wheel.remove(&id, conn.timer_wheel_index);
            if let Some(node) = conn.node {
                info!(self.logger, "Closing established connection"; "peer" => node.to_string());
                self.established.remove(&node);
            } else {
                info!(self.logger, "Closing unestablished connection");
            }
        }
    }

    fn broadcast_pings(&mut self) -> Result<()> {
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Ping::<T>;
        try!(msg.encode(&mut Encoder::new(&mut encoded))
             .chain_err(|| ErrorKind::EncodeError(None, None)));
        self.broadcast(encoded)
    }

    // Write encoded values to all connections and return the id of any connections with errors
    fn broadcast(&mut self, encoded: Vec<u8>) -> Result<()> {
        let mut errors = Vec::new();
        for (id, conn) in self.connections.iter_mut() {
            if !conn.members_sent {
                // This connection isn't connected yet
                continue;
            }
            match conn.writer.write(&mut conn.sock, Some(encoded.clone())) {
                Ok(false) => {
                    if let Err(e) = self.registrar.reregister(*id, &conn.sock, Event::Both) {
                        errors.push(registrar_error(e, *id, &conn.node));
                    }
                },
                Ok(true) => (),
                Err(e) => errors.push(write_error(e, *id, &conn.node))
            }
        }
        if errors.len() != 0 {
            return Err(ErrorKind::BroadcastError(errors).into());
        }
        Ok(())
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
               if let Err(e) = self.registrar.deregister(conn.sock) {
                   error!(self.logger, "Failed to deregister socket";
                          "id" => id, "peer" => conn.node.unwrap().to_string());
               }
           }
       }
   }
}

fn write_error(e: std::io::Error, id: usize, node: &Option<NodeId>) -> Error {
    let r: std::io::Result<()> = Err(e);
    r.chain_err(|| ErrorKind::WriteError(id, node.clone())).unwrap_err()
}

fn registrar_error(e: std::io::Error, id: usize, node: &Option<NodeId>) -> Error {
    let r: std::io::Result<()> = Err(e);
    r.chain_err(|| ErrorKind::RegistrarError(Some(id), node.clone())).unwrap_err()
}

