use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::fmt::Debug;
use std::time::Duration;
use libc::EINPROGRESS;
use net2::{TcpBuilder, TcpStreamExt};
use serde::{Serialize, Deserialize};
use msgpack::{Serializer, Deserializer};
use slog;
use amy::{Poller, Registrar, Notification, Event, FrameReader, FrameWriter, Sender, Receiver};
use members::Members;
use node_id::NodeId;
use executor::Executor;
use ferris::{Wheel, CopyWheel, Resolution};
use envelope::Envelope;
use orset::{ORSet, Delta};
use pid::Pid;
use errors::*;
use channel;
use super::{ClusterStatus, ClusterMsg, ExternalMsg, ClusterMetrics};

// TODO: This is totally arbitrary right now and should probably be user configurable
const MAX_FRAME_SIZE: u32 = 10*1024*1024; // 10 MB

const TICK_TIME: usize = 100; // milliseconds  - This must match the highest resolution of the timer wheels
const REQUEST_TIMEOUT: u64 = 5; // seconds
const POLL_TIMEOUT: usize = 5000; // ms

struct Conn {
    sock: TcpStream,
    node: Option<NodeId>,
    is_client: bool,
    members_sent: bool,
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
            reader: FrameReader::new(MAX_FRAME_SIZE),
            writer: FrameWriter::new(),
        }
    }
}

/// A struct that handles cluster membership connection and routing of messages to processes on
/// other nodes.
pub struct ClusterServer<T> {
    pid: Pid,
    node: NodeId,
    executor: Executor<T>,
    tx: Sender<ClusterMsg<T>>,
    rx: Receiver<ClusterMsg<T>>,
    timer_id: usize,
    timer_wheel: CopyWheel<usize>,
    listener: TcpListener,
    listener_id: usize,
    members: Members,
    connections: HashMap<usize, Conn>,
    established: HashMap<NodeId, usize>,
    poller: Poller,
    registrar: Registrar,
    logger: slog::Logger,
    metrics: ClusterMetrics
}

impl<'de, T: Serialize + Deserialize<'de> + Debug + Clone + Send + 'static> ClusterServer<T> {
    pub fn new(node: NodeId, logger: slog::Logger) -> ClusterServer<T> {
        let pid = Pid {
            group: Some("rabble".to_string()),
            name: "cluster_server".to_string(),
            node: node.clone()
        };
        let poller = Poller::new().unwrap();
        let mut registrar = poller.get_registrar();
        let (tx, rx) = registrar.channel().unwrap();
        let listener = TcpListener::bind(&node.addr[..]).unwrap();
        listener.set_nonblocking(true).unwrap();
        ClusterServer {
            pid: pid,
            node: node.clone(),
            executor: Executor::new(node.clone(), logger.new(o!("component" => "executor"))),
            tx: tx,
            rx: rx,
            timer_id: 0,
            timer_wheel: CopyWheel::new(vec![Resolution::HundredMs, Resolution::Sec]),
            listener: listener,
            listener_id: 0,
            members: Members::new(node),
            connections: HashMap::new(),
            established: HashMap::new(),
            poller: poller,
            registrar: registrar,
            logger: logger.new(o!("component" => "cluster_server")),
            metrics: ClusterMetrics::new()
        }
    }

    pub fn run(mut self) {
        info!(self.logger, "Starting");
        self.timer_id = self.registrar.set_interval(TICK_TIME).unwrap();
        self.listener_id = self.registrar.register(&self.listener, Event::Read).unwrap();
        loop {
            let notifications = self.poller.wait(POLL_TIMEOUT).unwrap();
            self.metrics.poll_notifications += notifications.len() as u64;
            if let Err(e) = self.handle_poll_notifications(notifications) {
                for id in e.kind().get_ids() {
                    self.close(id)
                }
                match *e.kind() {
                    ErrorKind::Shutdown(..) => {
                        info!(self.logger, e.to_string());
                        break;
                    },
                    _ => error!(self.logger, e.to_string())
                }
            }
        }
    }

    pub fn sender(&self) -> Sender<ClusterMsg<T>> {
        self.tx.clone()
    }

    fn receive_msgs(&mut self) -> Result<()> {
        while let Ok(msg) = self.rx.try_recv() {
            if let Err(e) = self.handle_cluster_msg(msg) {
                if let ErrorKind::Shutdown(..) = *e.kind() {
                    return Err(e);
                }
                error!(self.logger, e.to_string());
            }
        }
        Ok(())
    }

    fn handle_cluster_msg(&mut self, msg: ClusterMsg<T>) -> Result<()> {
        match msg {
            ClusterMsg::Join(node) => {
                self.metrics.joins += 1;
                self.join(node)?;
            }
            ClusterMsg::Leave(node) => {
                self.metrics.leaves += 1;
                self.leave(node)?;
            }
            ClusterMsg::Envelope(envelope) => {
                self.metrics.received_local_envelopes += 1;
                self.executor.send(envelope);
            }
            ClusterMsg::GetStatus(tx) => {
                self.metrics.status_requests += 1;
                self.get_status(tx)?;
            }
            ClusterMsg::Spawn(pid, process) => {
                self.executor.spawn(pid, process);
            }
            ClusterMsg::Stop(pid) => {
                self.executor.stop(pid);
            }
            ClusterMsg::RegisterService(pid, sender) => {
                self.executor.register_service(pid, sender);
            }
            ClusterMsg::Shutdown => {
                return Err(ErrorKind::Shutdown(self.pid.clone()).into());
            }
        }
        Ok(())
    }

    fn get_status(&self, tx: Box<channel::Sender<ClusterStatus>>) -> Result<()> {
        let status = ClusterStatus {
            members: self.members.all(),
            established: self.established.keys().cloned().collect(),
            num_connections: self.connections.len(),
            metrics: self.metrics.clone(),
            executor: self.executor.get_status()
        };
        tx.send(status).map_err(|_| "Failed to send cluster status".into())
    }

    fn send_remote(&mut self, envelope: Envelope<T>) -> Result<()> {
        if let Some(id) = self.established.get(&envelope.to.node).cloned() {
            trace!(self.logger, "send remote"; "to" => envelope.to.to_string());
            let mut encoded = Vec::new();
            let node = envelope.to.node.clone();
            try!(ExternalMsg::Envelope(envelope).serialize(&mut Serializer::new(&mut encoded))
                .chain_err(|| ErrorKind::EncodeError(Some(id), Some(node))));
            try!(self.write(id, Some(encoded)));
        }
        Ok(())
    }

    fn handle_poll_notifications(&mut self, notifications: Vec<Notification>) -> Result<()> {
        trace!(self.logger, "handle_poll_notification"; "num_notifications" => notifications.len());
        let mut errors = Vec::new();
        for n in notifications {
            let result = match n.id {
                id if id == self.listener_id => self.accept_connection(),
                id if id == self.timer_id => self.tick(),
                id if id == self.rx.get_id() => self.receive_msgs(),
                _ => self.do_socket_io(n)
            };

            if let Err(e) = result {
                if let ErrorKind::Shutdown(..) = *e.kind() {
                    return Err(e);
                }
                errors.push(e);
            }

            // We need to collect the envelopes due to borrow checker rules. NLL should fix this
            // eventually.
            let envelopes: Vec<_> = self.executor.remote_envelopes().collect();

            for envelope in envelopes {
                if let Err(e) = self.send_remote(envelope) {
                    errors.push(e);
                }
            }
        }
        if errors.len() != 0 {
            self.metrics.errors += errors.len() as u64;
            return Err(ErrorKind::PollNotificationErrors(errors).into());
        }
        Ok(())
    }

    fn do_socket_io(&mut self, notification: Notification) -> Result<()> {
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
        trace!(self.logger, "read"; "id" => id);
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
                info!(self.logger, "Got Members"; "id" => id, "from" => from.to_string());
                self.establish_connection(id, from, orset);
                self.check_connections();
            },
            ExternalMsg::Ping => {
                trace!(self.logger, "Got Ping"; "id" => id);
                self.reset_timer(id);
            }
            ExternalMsg::Envelope(envelope) => {
                self.metrics.received_remote_envelopes += 1;
                debug!(self.logger, "Got User Message";
                       "from" => envelope.from.to_string(),
                       "to" => envelope.to.to_string());
                self.executor.send(envelope);
            },
            ExternalMsg::Delta(delta) => {
                debug!(self.logger, "Got Delta mutator";
                       "id" => id, "delta" => format!("{:?}", delta));
                if self.members.join_delta(delta.clone()) {
                    try!(self.broadcast_delta(delta));
                }
            }
        }
        Ok(())
    }

    fn write(&mut self, id: usize, msg: Option<Vec<u8>>) -> Result<()> {
        trace!(self.logger, "write"; "id" => id);
        let registrar = &self.registrar;
        if let Some(mut conn) = self.connections.get_mut(&id) {
            if msg.is_none() {
                if conn.writer.is_writable() {
                    // The socket has just became writable. We need to re-register it as only
                    // readable, or it the event will keep firing indefinitely even if there is
                    // no data to write.
                    try!(registrar.reregister(id, &conn.sock, Event::Read)
                         .chain_err(|| ErrorKind::RegistrarError(Some(id), conn.node.clone())));
                }

                // We just got an Event::Write from the poller
                conn.writer.writable();
            }
            try!(conn_write(id, &mut conn, msg, &registrar));
        }
        Ok(())
    }

    fn reset_timer(&mut self, id: usize) {
        self.timer_wheel.stop(id);
        self.timer_wheel.start(id, Duration::from_secs(REQUEST_TIMEOUT));
    }

    /// Transition a connection from unestablished to established. If there is already an
    /// established connection between these two nodes, determine which one should be closed.
    fn establish_connection(&mut self, id: usize, from: NodeId, orset: ORSet<NodeId>) {
        self.members.join(orset);
        if let Some(close_id) = self.choose_connection_to_close(id, &from) {
            debug!(self.logger,
                   "Two connections between nodes. Closing the connection where \
                    the peer that sorts lower was the connecting client";
                    "peer" => from.to_string(), "id" => close_id);
            self.close(close_id);
            if close_id == id {
                return;
            }
        }
        debug!(self.logger, "Trying to establish connection"; "peer" => from.to_string(), "id" => id);
        if let Some(conn) = self.connections.get_mut(&id) {
            info!(self.logger, "Establish connection"; "peer" => from.to_string(), "id" => id);
            conn.node = Some(from.clone());
            self.timer_wheel.stop(id);
            self.timer_wheel.start(id, Duration::from_secs(REQUEST_TIMEOUT));
            self.established.insert(from, id);
        }
    }

    /// We only want a single connection between nodes. Choose the connection where the client side
    /// comes from a node that sorts less than the node of the server side of the connection.
    /// Return the id to remove if there is an existing connection to remove, otherwise return
    /// `None` indicating that there isn't an existing connection, so don't close the new one.
    fn choose_connection_to_close(&self, id: usize, from: &NodeId) -> Option<usize> {
        if let Some(saved_id) = self.established.get(from) {
            if let Some(saved_conn) = self.connections.get(&saved_id) {
                // A client connection always comes from self.node
                if (saved_conn.is_client && self.node < *from) ||
                    (!saved_conn.is_client && *from < self.node) {
                        return Some(*saved_id);
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

            for frame in conn.reader.iter_mut() {
                let mut decoder = Deserializer::new(&frame[..]);
                let msg = try!(Deserialize::deserialize(&mut decoder)
                               .chain_err(|| ErrorKind::DecodeError(id, node.clone())));
                output.push(msg);
            }
        }
        Ok(output)
    }

    fn join(&mut self, node: NodeId) -> Result<()> {
        let delta = self.members.add(node.clone());
        try!(self.broadcast_delta(delta));
        self.metrics.connection_attempts += 1;
        self.connect(node)
    }

    fn leave(&mut self, node: NodeId) -> Result<()> {
        if let Some(delta) = self.members.leave(node.clone()) {
            try!(self.broadcast_delta(delta));
        }
        Ok(())
    }

    fn connect(&mut self, node: NodeId) -> Result<()> {
        debug!(self.logger, "connect"; "to" => node.to_string());
        let sock = try!(TcpBuilder::new_v4().chain_err(|| "Failed to create a IPv4 socket"));
        let sock = try!(sock.to_tcp_stream().chain_err(|| "Failed to create TcpStream"));
        try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
        if let Err(e) = sock.connect(&node.addr[..]) {
            if e.raw_os_error().is_some() && *e.raw_os_error().as_ref().unwrap() != EINPROGRESS {
                return Err(e).chain_err(|| ErrorKind::ConnectError(node));
            }
        }
        try!(self.init_connection(sock, Some(node)));
        Ok(())
    }

    fn accept_connection(&mut self) -> Result<()> {
        while let Ok((sock, _)) = self.listener.accept() {
            self.metrics.accepted_connections += 1;
            debug!(self.logger, "accepted connection");
            try!(sock.set_nonblocking(true).chain_err(|| "Failed to make socket nonblocking"));
            let id = try!(self.init_connection(sock, None));
            try!(self.send_members(id));
        }
        Ok(())
    }

    fn init_connection(&mut self, sock: TcpStream, node: Option<NodeId>) -> Result<usize> {
        let id = try!(self.registrar.register(&sock, Event::Read)
                      .chain_err(|| ErrorKind::RegistrarError(None, None)));
        debug!(self.logger, "init_connection()";
               "id" => id, "is_client" => node.is_some(), "peer" => format!("{:?}", node));
        let is_client = node.is_some();
        let conn = Conn::new(sock, node, is_client);
        self.timer_wheel.start(id, Duration::from_secs(REQUEST_TIMEOUT));
        self.connections.insert(id, conn);
        Ok(id)
    }

    fn send_members(&mut self, id: usize) -> Result<()> {
        let encoded = try!(self.encode_members(id));
        let registrar = &self.registrar;
        if let Some(mut conn) = self.connections.get_mut(&id) {
            info!(self.logger, "Send members"; "id" => id);
            try!(conn_write(id, &mut conn, Some(encoded), &registrar));
            conn.members_sent = true;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        trace!(self.logger, "tick");
        let expired = self.timer_wheel.expire();
        self.deregister(expired);
        try!(self.broadcast_pings());
        self.check_connections();
        self.executor.process_timeouts();
        Ok(())
    }

    fn encode_members(&self, id: usize) -> Result<Vec<u8>> {
        let orset = self.members.get_orset();
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Members::<T> {from: self.node.clone(), orset: orset};
        try!(msg.serialize(&mut Serializer::new(&mut encoded))
             .chain_err(|| ErrorKind::EncodeError(Some(id), None)));
        Ok(encoded)
    }

    fn deregister(&mut self, expired: Vec<usize>) {
        for id in expired.iter() {
            warn!(self.logger, "Connection timeout"; "id" => *id);
            self.close(*id);
        }
    }

    /// Close an existing connection and remove all related state.
    fn close(&mut self, id: usize) {
        if let Some(conn) = self.connections.remove(&id) {
            let _ = self.registrar.deregister(&conn.sock);
            self.timer_wheel.stop(id);
            if let Some(node) = conn.node {
                // Remove established connection if it matches this id
                if let Some(established_id) = self.established.remove(&node) {
                    if established_id == id {
                        info!(self.logger, "Closing established connection";
                              "id" => id,"peer" => node.to_string());
                        return;
                    }
                    // The established node didn't correspond to this id, so put it back
                    self.established.insert(node, established_id);
                }
            }
            info!(self.logger, "Closing unestablished connection"; "id" => id);
        }
    }

    fn broadcast_delta(&mut self, delta: Delta<NodeId>) -> Result<()> {
        debug!(self.logger, "Broadcasting delta"; "delta" => format!("{:?}", delta));
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Delta::<T>(delta);
        try!(msg.serialize(&mut Serializer::new(&mut encoded))
             .chain_err(|| ErrorKind::EncodeError(None, None)));
        self.broadcast(encoded)
    }

    fn broadcast_pings(&mut self) -> Result<()> {
        let mut encoded = Vec::new();
        let msg = ExternalMsg::Ping::<T>;
        try!(msg.serialize(&mut Serializer::new(&mut encoded))
             .chain_err(|| ErrorKind::EncodeError(None, None)));
        self.broadcast(encoded)
    }

    // Write encoded values to all connections and return the id of any connections with errors
    fn broadcast(&mut self, encoded: Vec<u8>) -> Result<()> {
        let mut errors = Vec::new();
        let registrar = &self.registrar;
        for (id, mut conn) in self.connections.iter_mut() {
            if !conn.members_sent {
                // This connection isn't connected yet
                continue;
            }
            if let Err(e) = conn_write(*id, &mut conn, Some(encoded.clone()), &registrar) {
                errors.push(e)
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

        // If this node is no longer a member of the cluster disconnect from all nodes
        if !all.contains(&self.node) {
            return self.disconnect_all();
        }

        // Pending, Client connected, or established server side connections
        let known_peer_conns: HashSet<NodeId> =
            self.connections.iter().filter_map(|(_, conn)| conn.node.clone()).collect();

        let to_connect: Vec<NodeId> = all.difference(&known_peer_conns)
                                       .filter(|&node| *node != self.node).cloned().collect();

        let to_disconnect: Vec<NodeId> = known_peer_conns.difference(&all).cloned().collect();

        trace!(self.logger, "check_connections";
               "to_connect" => format!("{:?}", to_connect),
               "to_disconnect" => format!("{:?}", to_disconnect));

        for node in to_connect {
            self.metrics.connection_attempts += 1;
            if let Err(e) = self.connect(node) {
                warn!(self.logger, e.to_string());
            }
        }

        self.disconnect_established(to_disconnect);
    }

    fn disconnect_all(&mut self) {
        self.established = HashMap::new();
        for (id, conn) in self.connections.drain() {
            self.timer_wheel.stop(id);
            if let Err(e) = self.registrar.deregister(&conn.sock) {
                error!(self.logger, "Failed to deregister socket";
                       "id" => id, "peer" => format!("{:?}", conn.node),
                       "error" => e.to_string());
            }
        }
    }

    fn disconnect_established(&mut self, to_disconnect: Vec<NodeId>) {
        for node in to_disconnect {
            if let Some(id) = self.established.remove(&node) {
                let conn = self.connections.remove(&id).unwrap();
                self.timer_wheel.stop(id);
                if let Err(e) = self.registrar.deregister(&conn.sock) {
                    error!(self.logger, "Failed to deregister socket";
                           "id" => id, "peer" => conn.node.unwrap().to_string(),
                           "error" => e.to_string());
                }
            }
        }
    }
}

fn conn_write(id: usize,
              conn: &mut Conn,
              msg: Option<Vec<u8>>,
              registrar: &Registrar) -> Result<()>
{
        let writable = try!(conn.writer.write(&mut conn.sock, msg).chain_err(|| {
            ErrorKind::WriteError(id, conn.node.clone())
        }));
        if !writable {
            return registrar.reregister(id, &conn.sock, Event::Both)
                .chain_err(|| ErrorKind::RegistrarError(Some(id), conn.node.clone()));
        }
        Ok(())
    }

