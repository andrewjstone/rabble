# Introduction
This document details the internal structure and design of rabble, as well as justifications for
some of the design decisions. It highlights the major abstractions and how they fit into the bigger
picture of building a large scale clustered network application. An attempt will be made to clarify
where certain decisions were made for expedience, and what may be done in the future to enhance
Rabble.

Rabble provides a location transparent actor system via a fully connected mesh of
[nodes](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node.rs) identified by
a unique [NodeId](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node_id.rs).
Nodes are joined together to form clusters upon which actors can run and communicate via sending
messages.  Rabble supports two types of actors in the system: [lightweight
processes](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/process.rs) and
thread based
[services](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/service.rs). Each
actor has a globally unique
[Pid](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/pid.rs) used to identify
and send messages to it.

# Abstractions

### Messages
Rust is a statically typed programming language and contains statically typed channels for
communication between threads. Furthermore, any objects, such as processes, stored in a collection,
must be of the same type. This presents a unique challenge for building an actor system, since
traditionally actor systems allow sending and receiving arbitrary messages. Rabble was therefore
presented with 2 primary choices for messaging between actors. Use a single type of message for all
actors or use dynamically typed messages via [Any](https://doc.rust-lang.org/std/any/) or [Trait
Objects](https://doc.rust-lang.org/stable/book/trait-objects.html).

Any dynamic type sent over a channel must be boxed and therefore requires an allocation, while a
static type can simply be copied. Furthermore, dynamic types require runtime reflection. They also
add to the implementation complexity of the system, because serialization for sending between nodes
requires direct implementation, rather than derivation via compiler plugins. However, these types do
provide open extensibility and some sort of data hiding, since actors will only attempt to downcast
messages they know about.

The performance and complexity cost of dynamic types in Rust, as well as the loss of static type
checking capabilities outweighs the benefits of open extensibility. Therefore every message sent
between actors in rabble is a statically typed, parameterized
[Msg](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/msg.rs). The `User(T)`
variant of `Msg` contains the user defined static type, which once again, is shared among *all*
actors in the system.

Now, there is a caveat to the above description. If a user really desires open extensibility and
doesn't care about the performance penalty or implementation complexity, they can use Boxed types,
as long as they implement the required traits manually, namely: `Debug, Clone, Eq,
PartialEq, RustcEncodable, and RustcDecodable`.

Note lastly, that this restriction on a single type for messages only applies to messages sent
between actors. Client APIs may use their own message types.


### Processes
Processes are intended as the primary actor type to be utilized when building systems
upon rabble.  Processes implement the `process` trait which consists of an associated type, `Msg`,
and a single method `handle`, shown below. The associated type is the type parameter to the `Msg`
enum, which is the single type shared among actors as described above.

```Rust
pub trait Process : Send {
  type Msg: Encodable + Decodable + Debug + Clone;
  fn handle(&mut self,
            msg: Msg<Self::Msg>,
            from: Pid, correlation_id:
            Option<CorrelationId>,
            output: Vec<Envelope<Self::Msg>>);
}
```

Processes contain an internal state that can be mutated when a message is handled. Processes can
only responed to messages, and do not generate output without input. Any output messages to actors
in response to the input message are not sent directly over channels but are instead packaged into
[envelopes](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/envelope.rs) and
pushed onto an output Vec.

The choice to return envelopes inside a mutable Vec of envelopes is an interesting one, so a short
discussion of why this was chosen is in order. A key goal of rabble is to enable easy testability of
indivdiual processes and protocols involving those processes. While other actor systems allow
processes to directly send messages while running a callback, this side-effect behavior is very hard
to test. In a traditional actor language like Erlang, these side-effects manifest as non-determinism
in the ordering of messages due to scheduling behavior of the processes. Re-running a failing test
often results in a different messaging order making the failure hard to reproduce and the root cause
hard to discover. By making the interface to each process a function call that only modifies
internal state and returns envelopes, we can carefully control the ordering of all messages between
processes by a test and can even allow dropping, delaying or re-ordering of those messages in ways
specific to the test itself.  This allows for full determinism specified by the test, and allows
building interactive debuggers that can literally step through the messages sent in a system. Note
that while the order of test messages is deterministic and tests are repeatable, covering the entire
state space is still just as hard as in traditional actor systems. Therefore, long-running
simulations of multiple schedules in a [quickcheck](https://github.com/BurntSushi/quickcheck) like
manner, or exhaustive [model checking](https://en.wikipedia.org/wiki/Model_checking) with or without
[partial order reduction](https://en.wikipedia.org/wiki/Partial_order_reduction) of state space is
recommended.

### Executor
Each process receives a messages sent to it when its `handle` method gets called, and returns any
output envelopes. But processes are just objects and do not have their own thread of control, so
what is the mechanism that calls a process's handle method and routes responses to other processes?
This component is called the
[Executor](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor.rs) and is
responsible for routing all messages on a single node to their corresponding processes, and [calling
the process's handle
method](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor.rs#L117). Any
messages destined for actors on another node will be sent over a channel to the cluster server
which will forward the message. The cluster server will be described in the next section.

For implementation expediency and practical purposes, the executor currently runs in a single
thread. All processes are stored in a [HashMap keyed by their
Pids](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor.rs#L21). A
single async channel receiver receives
[ExecutorMsg](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor_msg.rs)s
in a [loop](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor.rs#L56)
that contain both requests for the executor, as well as envelopes that need to be sent to local
actors in the system. Note that the executor not only calls processes' handle method, it also
forwards envelopes over channels to any service that has it's Pid registered. Services will be
described in a later section.

### Cluster Server
The [cluster
server](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/cluster_server.rs)
maintains TCP connections to other nodes and manages cluster membership state. It serves as a bridge
for routing messages between actors on different nodes. It [receives
messages](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node.rs#L52-L73)
from the executor or
service threads which need to be serialized and sent to processes on other nodes when the
appropriate peer sockets are writable. It also receives [notifications from the kernel
poller](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/cluster_server.rs#L175-L192) that
sockets are available to be read or written, or a timer has fired. [Peer sockets are read from,
messages are deserialized, and then forwarded to the appropriate local actor via the executor
channel](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/cluster_server.rs#L215-L260).
Timer notifications are likewise forwarded to local processes.  Services manage their own timers.

Finally, there needs to be some way of establishing connections and configuring the cluster network.
A [cluster membership
API](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node.rs#L52-L73) exists
as part of the [Node](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node.rs) object that sends messages to the cluster server instructing it to change its
membership. Connections will then be established or torn down asynchronously. [Cluster
status](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/cluster_status.rs)
information is retrieved in the [same
manner](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/node.rs#L111-L117).
Note that the cluster membership API is not run in it's own thread, but is run in the context of the
caller.

### Services
For constructing I/O bound network protocols, lightweight processes are an excellent choice.
However, since all processes are executed inside a single thread, doing a lot of CPU intensive work,
or making a blocking system call will delay other processes from running and cause latency spikes.
What we need is a way for processes to outsource blocking or expensive operations to other threads.
[Services](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/service.rs) provide
this mechanism. Services are also actors in the system and can send and receive messages with
processes and other services. They enable this by [registering a sender and a Pid with the
executor](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/service.rs#L37). The
executor can then appropriately [route messages to services instead of processes based on the
destination
Pid](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/executor.rs#L106-L109).

Services are capable of interacting directly with the network, as well as actors. This enables users
to implement admin and API servers to manage and interact with applications running across a rabble
cluster. Note that because services can access the network directly, they can use whatever protocol
or message format the user desires, and do not need to use the same message type as actors.

In order to create a service, a user must implement a [service
handler](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/service_handler.rs)
to specialize the service. The simplest type of service handler, the [thread
handler](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/thread_handler.rs),
only handles envelopes from other actors and performs no network interaction. This type of handler
is useful for running expensive computations or performing file operations. An example
implementation can be found
[here](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/tests/basic.rs#L26-L29)
Note that the callback receives a node as well as an envelope. This allows it to send replies or
notifications to other actors.

The second major use for a service, as described above, is for server endpoints. While any network
protocol can be used for this, TCP is a common choice. Therefore a [TCP
handler](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/tcp_server_handler.rs)
has already been implemented in Rabble. This handler supports generic encoding and decoding of
messages by implementing the [Serialize
trait](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/serialize.rs).
A [MsgPack](http://msgpack.org/index.html) based implementation is provided
[here](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/msgpack_serializer.rs).

In this TCP handler, each connection is independent of other connections, and is structured so that
the user only has to provide callback functions to send and receive messages to and from actors and
the network. If a user wants to use the TCP handler provided by rabble they must implement a
[connection
handler](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/src/connection_handler.rs).
An example of implementing a connection handler for MsgPack based messages in a chain replication
application exists
[here](https://github.com/andrewjstone/rabble/blob/e1474eda584f3c278322ce21d33d56e6e30f639f/tests/utils/api_server.rs#L45-L109).
Note that the connection handler trait is not specific to TCP and can be re-used for other
connection based protocols such as SCTP.

# Limitations

 * Currently there is no backpressure provided by the system. An
   [issue](https://github.com/andrewjstone/rabble/issues/2)  has been opened.
 * Operability is limited by the lack of metrics and status information. An
   [issue](https://github.com/andrewjstone/rabble/issues/4) has been opened
   for this as well.
 * Pids and NodeIds use strings for identifiers. Due to the amount of comparisons on both of these
   types, this is extremely inefficient and wasteful. Both Pids and NodeIds should be converted to
   [Atoms](http://stackoverflow.com/questions/36023947/how-do-erlang-atoms-work/36025280). An
   [issue](https://github.com/andrewjstone/rabble/issues/5) has been created.

