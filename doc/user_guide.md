# Introduction
Rabble is useful for building distributed, clustered applications where actors can run on different
[Nodes](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/node.rs) and
communicate over the network. This allows for easier implementation of distributed algorithms based
around asynchronous message passing between processes. Actors in rabble are primarily lightweight
[processes](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/process.rs) that
receive and send messages. Thread based
[services](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/service.rs) provide
a way to run computation heavy tasks, interact with the file system, or implement an API server
while retaining the capability to send to and receive messages from other processes and services.

This guide will show how to get started building a distributed system in rabble from the ground up.
The reader will learn how to create a node, join nodes together, spawn processes to act as peers
in a distributed system, and create an api service to allow interaction with that system.

# What are we building?
Our example should be complete enough to show off most features of rabble, while not shrouding the
basics with the complexity of the algorithm implementation. In light of this,
we will build a very simple and utterly fault intolerant replicated counter. The service will have 3 nodes,
with a replica on each node. The first node is the primary node, and has a TCP server that can take
requests to either increment the counter or get the current count. When an increment request is received it
will be sent to the primary replica on the same node which will then forward the request to the two
backup replicas and wait for the replies from both replicas. When the primary replica has received the replies it
will go ahead and send a message to the tcp server so it can respond to the client. Requests for the
current count are answered directly from the primary replica.

Note that this example is simplified in some major ways, and is an absolutely terrible way to build
a distributed counter. It assumes that:

  1. The network is reliable. Nodes will never become partitioned or lose connectivity.
  2. The network is not asynchronous, and messages are sent in bounded time. In the world of this
     example, any messages communication will occur without delay or timeout.
  3. Nodes will never crash. Replicas will always maintain the same position in the primary/backup
     relationship and will always have up to date data.

It probably assumes a bunch more
[fallacies](http://www.lasr.cs.ucla.edu/classes/188_winter15/readings/fallacies.pdf) than those, but
that's enough to show that you shouldn't build a production system in this manner, and that this is
only an example to explain how to use Rabble.

# Creating your nodes
Each node needs a unique
[NodeId](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/node_id.rs). A node
also needs a msg type for messages sent between actors. All actors can only send and receive a
single message type. You can read more about why
[here](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/doc/architecture.md#messages).
A node can then be started with a call to
[rabble::rouse](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/lib.rs#L80).

```Rust
use rabble::NodeId;

// The message shipped between actors in the system. It must implement these derived traits.
// RustcEncodable and RustcDecodable provide serialization capability to arbitrary formats.
#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
enum CounterMsg {
    Increment,
    Ok, // Backup successfully received the Increment
    GetCount,
    Count(usize),
}

let node_ids = create_node_ids(3);

/// Each call to rabble::rouse spawns a few threads and returns their `JoinHandle`s along with the node.
/// The handles should be joined at some point later in the code. None as the second parameter to
/// rouse means just use the standard logger.
let (node, handles) = node_ids.cloned().into_iter().fold((Vec::new(), Vec::new()), |(mut nodes, mut handles), node_id| {
    let (node, handle_list) = rabble::rouse::<CounterMsg>(node_id, None);
    nodes.push(node);
    handles.extend(handle_list);
    (nodes, handles)
});

/// Create N node ids with names node1,node2,... and unique IP addresses. Don't create more than 9 :D
pub fn create_node_ids(n: usize) -> Vec<NodeId> {
    (1..n + 1).map(|n| {
        NodeId {
            name: format!("node{}", n),
            addr: format!("127.0.0.1:1100{}", n)
        }
    }).collect()
}
```

# Creating and starting 3 replicas

We now have 3 nodes up and running. We want to implement a replica process and then start one on
each node.

First let's create 3 Pids, one for each process, using the ``node_ids`` created previously. Note that
the `group` member of a pid can be used for a variety of reasons including multi-tenancy. For now,
let's just leave it blank.

```Rust
    let pids = ["replica1", "replica2", "replica3"].iter().zip(node_ids).map(|(name, node_id)| {
        Pid {
            name: name.to_string(),
            group: None,
            node: node_id.clone()
        }
    }).collect()
```

Now we need to define our replica type and implement the counter process. Note that the messages
received by a process are of type
[Msg](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/msg.rs) which is
paramterized by the `CounterMsg`. This allows receipt of system data as well as user defined types.
For now though, we will just concern ourself with the `User(T)` variant of the `Msg` enum.
Additionally, each message has a corresponding
[CorrelationId](https://github.com/andrewjstone/rabble/blob/b8ae3d7915542c5969196b3a799bf83ef53f011e/src/correlation_id.rs)
used to match requests with responses. Any received messages should put the correlation id in the
outgoing envelopes.

```Rust
pub struct Counter {
    pid: Pid,
    primary: bool,
    backups: Vec<Pid>,
    count: usize,
    output: Vec<Envelope<CounterMsg>>,

    // We have to wait for both backup replies before responding to the client
    backup_replies: HashMap<CorrelationId, usize>
}

impl Counter {
  pub fn new(pid: Pid, primary: Pid, backups: Vec<Pid>) -> Counter {
      // Size the output vector for the expected number of outgoing messages
      let size = if pid == primary {
          2
      } else {
          1
      };

      Counter {
        pid: pid,
        primary: primary == pid,
        backups: backups,
        count: 0,
        output: Vec::with_capacity(size),
        backup_replies: HashMap::new()
      }
  }
}

impl Process for Counter {
    // Each process needs a type. We defined it above. It's the one we used to paramaterize the call
    // to rabble::rouse()
    type Msg = CounterMsg;

    // Each process must implement a single method, `handle`.
    fn handle(&mut self, msg: Msg<CounterMsg>, from: Pid, correlation_id: Option<CorrelationId>)
        -> &mut Vec<Envelope<CounterMsg>>
    {
        match msg {
          Msg::User(CounterMsg::Inc) => {
              self.count += 1;
              if self.primary {
                  // Send the increment to the two backups
                  // For now assume correlation_id is a `Some`
                  self.backup_replies.insert(correlation_id.as_ref().unwrap().clone(), 0);
                  for &b in self.backups {
                      let msg = Msg::User(CounterMsg::Inc);
                      let envelope = Envelope::new(b.clone(), self.pid.clone(), msg, correlation_id);
                      self.output.push(envelope);
                  }
              } else {
                  // Respond to the primary
                  let reply = Msg::User(CounterMsg::Ok);
                  let envelope = Envelope::new(from, self.pid.clone(), reply, correlation_id);
                  self.output.push(envelope);
              }
          },
          Msg::User(CounterMsg::GetCount) => {
              // Only the primary gets this message
              let reply = Msg::User(CounterMsg::Count(self.count));
              let envelope = Envelope::new(from, self.pid.clone(), reply, correlation_id);
              self.output.push(envelope);
          },
          Msg::User(CounterMsg::Ok) => {
              // Increment the backup_replies. Once we have received both, reply to the client
              // Do this in a block to limit the borrow scope
              let count = {
                  let count = self.backup_replies.get_mut(correlation_id.as_ref().unwrap()).unwrap();
                  *count += 1;
                  *count
              };

              if count == 2 {
                  self.backup_replies.remove(correlation_id.as_ref().unwrap());
                  // Send to the original requester, not the sender. For now assume the correlation_id
                  // is a Some(id). It has to be for any chained req/response to work properly.
                  let to = correlation_id.as_ref().unwrap().pid.clone();
                  let reply = CounterMsg::Ok;
                  let envelope = Envelope::new(to, self.pid.clone(), reply, correlation_id);
                  self.output.push(envelope);
              }
          },
          _ => unreachable!()
        }
        &mut self.output
    }
}
```

Now let's start the replicas so that they can receive and send messages.

```Rust
let primary = pids[0].clone();
let backups = vec![pids[1].clone, pids[2].clone()];
for pid in pids {
    // Processes can be any type that implements Process, so create a trait object with Box::new()
    let replica = Box::new(Counter::new(pids[i].clone(), primary.clone(), backups.clone()));
    // Start the replica on the correct node
    nodes[i].spawn(&pids[i], replica).unwrap();
}
```

# Join the nodes
We need to join the nodes together into a cluster. Note that this is an operation that should most
likely be exposed to the end user via an Admin server. For now though, we are just going to use the
Rabble [Node
API](https://github.com/andrewjstone/rabble/blob/b8ae3d7915542c5969196b3a799bf83ef53f011e/src/node.rs#L52-L65)
to do the join.

In order to know when the nodes have been joined, we need to have some way of checking the cluster
state and getting responses back to our requests. Normally this would be done in an admin service,
but for now we can just register a channel for our test and poll on it.

```Rust
nodes[0].join(&nodes[1].id).unwrap();
nodes[0].join(&nodes[2].id).unwrap();

// Create a Pid for our "test service". This is used to register a channel so that we can receive
// responses to requests.
let test_pid = Pid {
    name: "test-runner".to_string(),
    group: None,
    node: node_ids[0].clone()
};

// We create an amy channel so that we can pretend this test is a service.
// We register the sender and our pid with node1 so that we can check the responses to admin calls
// like node.cluster_status().
let mut poller = Poller::new().unwrap();
let (test_tx, test_rx) = poller.get_registrar().channel().unwrap();
nodes[0].register_service(&test_pid, &test_tx).unwrap();

let start = SteadyTime::now();
loop {
      // Create a CorrelationId so that the responses to our requests get sent back on the right channel
      let correlation_id = CorrelationId::pid(test_pid.clone());

      // Send a ClusterStatus request to the cluster server on node1.
      nodes[0].cluster_status(correlation_id).unwrap();

      // Poll on the test channel for a response. We should only get a ClusterStatus response
      let _ = poller.wait(5000).unwrap();
      let envelope = test_rx.try_recv().unwrap();

      // Match on the msg and see if both backups are currently connected to node1
      if let Msg::ClusterStatus(ClusterStatus{connected, ..}) = envelope.msg {
        if connected.len() == 2 {
            println!("{:#?}", connected);
            println!("Cluster connected in {} ms", (SteadyTime::now() - start).num_milliseconds());
            break;
        }
      }
}
```

# Creating an API Service
Now we have 3 nodes up, with a counter process on each one. We hacked our way through the cluster
setup, but now we want to learn how to build a service so that we can present both admin and API
servers to network clients. Since we've already joined the nodes, we'll focus on building an API
server here. All services must implement the [ServiceHandler
trait](https://github.com/andrewjstone/rabble/blob/b8ae3d7915542c5969196b3a799bf83ef53f011e/src/service_handler.rs).

Our API service will use 4 byte framed MsgPack encoded messages over TCP and will use the
already built
[TcpServerHandler](https://github.com/andrewjstone/rabble/blob/b8ae3d7915542c5969196b3a799bf83ef53f011e/src/tcp_server_handler.rs).
This service isolates connections from each other and routes messages to the correct connection.
Connection handlers themselves are user specified and can be customized for the specific
application. Therefore instead of writing a service handler directly we will instead need to
implement a
[ConnectionHandler](https://github.com/andrewjstone/rabble/blob/b8ae3d7915542c5969196b3a799bf83ef53f011e/src/connection_handler.rs).

Each connection handler has 2 message types that must be defined. One is for the actors in the
system, which is the `CounterMsg` we've been using in the rest of the example. The other is the
message sent between the client and the API server. In almost every case these messages will differ,
but for our purposes they can be the same message.

There are 3 callback functions to implement for a ConnectionHandler. `new()` is called with
the pid of the service running the service handler (which calls the connection handler), and the
unique id of the connection for use in correlation ids. ``handle_envelope()``is called when an actor msg
message is sent to the connection handler. In general this occurs when a reply to a client request
comes back to the handler. This reply is then bundled into the `ConnectionMsg::Client` variant and
returned so it can be sent back on the client connection. ``handle_network_msg()`` gets called when a
new message is received from the client. These requests are packed into Envelopes and returned as
`ConnectionMsg::Envelope` variants so they can be routed to actors.

```Rust
pub struct ApiServerConnectionHandler {
    pid: Pid,
    counter_pid: Pid,
    id: usize,
    total_requests: usize,
    output: Vec<ConnectionMsg<ApiServerConnectionHandler>>
}

impl ConnectionHandler for ApiServerConnectionHandler {
    type Msg = CounterMsg;
    type ClientMsg = CounterMsg;

    fn new(pid: Pid, id: usize) -> ApiServerConnectionHandler {
        let counter_pid = Pid {
            name: "replica1".to_string(),
            group: None,
            node: pid.node_id.clone()
        };

        ApiServerConnectionHandler {
            pid: pid,
            counter_pid: counter_pid,
            id: id,
            total_requests: 0,
            output: Vec::with_capacity(1)
        }
    }

    fn handle_envelope(&mut self, envelope: Envelope<CounterMsg>)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        let Envelope {msg, correlation_id, ..} = envelope;
        // Envelopes destined for a connection handler must have a correlation id
        let correlation_id = correlation_id.unwrap();

        match msg {
            Msg::User(counter_msg) =>
              self.output.push(ConnectionMsg::ClientMsg(counter_msg, correlation_id));

            // Requests can timeout as well. Our client message should contain a Timeout variant.
            Msg::Timeout => ...,

            _ => ... /// ignore other messaages for now
        }
    }

    fn handle_network_msg(&mut self, msg: CounterMsg)
        -> &mut Vec<ConnectionMsg<ApiServerConnectionHandler>>
    {
        // Our client and actor messages are the same, so just forward to the counter process.
        // Note that in a real system, either the counter Pid would be passed in from the client, known
        // a-priori, or learned via an envelope in `handle_envelope`. For now we just know it
        // a-priori.
        let msg = Msg::User(msg);
        let correlation_id = CorrelationId::request(self.pid.clone(), self.id, self.total_requests);
        self.total_requests += 1;
        let envelope = Envelope::new(self.counter_pid.clone(), self.pid.clone(), msg, Some(correlation_id));
        self.output.push(ConnectionMsg::Envelope(envelope));
        &mut self.output
    }
```

Now that we've created the connection handler for our API server, we need to give the service a Pid and start the server.

```Rust
    let server_pid = Pid {
        name: "api-server".to_string(),
        group: None,
        node: nodes[0].id.clone()
    };

    /// Create a TcpServerHandler that listens on "127.0.0.1:11001", has a 5 second request timeout
    /// and no connection timeout.
    let handler: TcpServerHandler<ApiServerConnectionHandler, MsgpackSerializer<CounterMsg>> =
        TcpServerHandler::new(server_pid.clone(), "127.0.0.1:11001", 5000, None);
    let mut service = Service::new(server_pid, nodes[0].clone(), handler).unwrap();

    // Services need to run in their own thread
    let h = thread::spawn(move || {
        service.wait();
    });
```

# Timers

The guide so far has explained how to implement a system using rabble. It hit all of the major
points. However, in assuming a bounded, reliable network, the example ignored worrying about lost or
delayed messages. In reality, distributed systems must take account of this by setting a timer for
each request. If the timer expires, then the user is alerted of the timeout. Whether the request
succeeded or failed is indeterminate. This is an unfortunate fact of nature. Rabble allow users to
add timers for all requests from within a process or service. (Note that the TcpServerHandler
automatically manages request timeouts, so it is unneccessary to use this facility for that
purpose.)

Timers are tied to a given process and correlation id, and are declared in milliseconds.
Currently the maximum timer length is 59 minutes, and the minimum timer resolution is 10ms. Timers
under one second are rounded to the higher 10ms, timers of 1 second to 59 seconds are rounded to
the higher second, and timers of 1 minute or more are rounded to the higher minute. This behavior
is based on the hierarchical timer wheel implementation in
[ferris](https://github.com/andrewjstone/ferris).

Additionally, processes may want to return messages or set timers on startup. For this reason, there
is an optional
[init()](https://github.com/andrewjstone/rabble/blob/cluster-server-rebase/src/process.rs#L12-L14)
callback that can be implemented for processes. The example below will show the impelmentation of a
simple test process that starts a 100ms timer in `init()` by responding with a message destined for
the executor, and then gets a callback `Msg::Timeout` in `handle`.

```Rust
struct TestProcess {
    pid: Pid,
    executor_pid: Option<Pid>,
    output: Vec<Envelope<()>>
}

impl Process for TestProcess {
    type Msg = ();

    fn init(&mut self, executor_pid: Pid) -> Vec<Envelope<()>> {
        self.executor_pid = Some(executor_pid);

        // Start a timer with a 100ms timeout and no correlation id. We don't need one since there is
        // only one timer in this example. In practice timers should almost always have CorrelationIds.
        vec![Envelope::new(self.executor_pid.as_ref().unwrap().clone(),
                           self.pid.clone(),
                           Msg::StartTimer(100),
                           None)]
    }

    fn handle(&mut self,
              msg: Msg<()>,
              from: Pid,
              correlation_id: Option<CorrelationId>) -> &mut Vec<Envelope<()>>
    {
      assert_eq!(from, *self.executor_pid.as_ref().unwrap());
      assert_eq!(msg, Msg::Timeout);
      assert_eq!(correlation_id, None);
      &mut self.output
    }
}
```


