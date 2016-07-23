# Introduction
The goal of rabble is to provide a fully connected mesh of nodes that serve to allow location
independent actor communication ala Erlang. An actor in rabble is known as a process and has a
single method, `receive`, that receives messages from other actors or system threads. Processes are
externally pure in that they are only supposed to modify their internal mutable state and then return
any outgoing messages from the receive call. While processes should not perform I/O, this is is not
enforced via the type system as in Haskell, since there isn't a way to do that in Rust. By allowing
processes to only perform compute operations and communicate via sending messages we vastly simplify
how they can be composed, scheduled and tested.

Since processes cannot do I/O directly, there must be system threads that perform this I/O as
needed. File I/O is not built directly into rabble, but network I/O for intracluster communication
is. Systems building on top of rabble are expected to provide their own set of I/O threads as needed
to provide the services they need. The remainder of this document will describe how rabble is
internally constructed and how programs should use the cluster and process abstractions provided by
rabble in order to build clustered distributed systems.

# Architecture

### Processes and the executor
In order to simplify construction of the first version of rabble, all processes are run in a single
`executor` thread. A single channel receiver is run in that thread that receives `Envelopes`
consisting of the to Pid, the from Pid and the message being sent. Pids are process identifiers that
consist of a name, a node, and a group that can be used for multi-tenancy purposes. Processes are
stored in a hashmap keyed by their Pids and when an envelope is received destined for a Pid, the
process is retrieved from the map and the receive method is called with the message and Pid of the
sender. Any outgoing envelopes are returned from receive. Outgoing envelopes destined for processes
on the same node are sent on the channel sender to the executor thread. Outgoing envelopes intended
for another node are send on the encoder channel to the cluster server thread, where they will be
serialized and transmitted over a TCP connection to the proper node.

### Cluster Server
The cluster server maintains TCP connections to other nodes and manages cluster membership state. It
receives messages from the executor thread which need to be serialized and sent to processes on
other nodes when the appropriate peer sockets are writable. It also receives notifications from the
network poll thread that sockets are available to be read or written, or a timer has fired. Peer
sockets are read from, messages are deserialized, and then forwarded to the appropriate local
process via the executor channel. Timer notifications are likewise forwarded to local processes.
Finally, there needs to be some way of establishing connections and configuring the cluster network.
A cluster membership API exists that sends messages to the cluster server instructing it to change
it's membership. Connections will then be established or torn down asynchronously. Membership status
information is retrieved in the same manner. Note that the cluster membership API is not run in it's
own thread, but is run in the context of the caller.

### Poll thread
The poll thread has one responsibility. It waits for events from the kernel poller and forwards
notifications of these events to the cluster server. Registration of events with the kernel poller
is done from the cluster server, so the poll thread does not receive any messages from other
threads.

### File I/O
File I/O on most operating systems is handled with blocking operations. Furthermore, concurrent
operations against files on spinning disks can be wildly unpredictable and inefficient. For this
reason there will be a separate thread to handle file I/O. The files and decisions about buffer
sizes, and when to sync are usually program and even platform dependent. All file operations will be
returned from processes' receive methods in envelopes destined for the file I/O thread. This thread
will be user implemented and results will be sent back to the executor or cluster server threads as
appropriate and forwarded to the proper process. As some programs will execute on hardware with
multiple disks it may be wise to parallelize these operations. This can be done by forwarding any
operations to separate threads in an application specific manner.

### Administration
Administration of rabble consists mainly of two types of operations:
 * Establishing a cluster via the cluster membership API
 * Application specific operations and status

Since administration of a clustered service relies upon application specific operations, the
implementation will be left up to the application. This can be handled via a standard blocking TCP
server with a thread per connection model, or an async server using Mio or Amy. Since the likely
number of concurrent clients should be low for administrative operations, using the thread per
connection model should make things easier to implement. If only one user will be acting
as administrator at a time, using standard input and output is another viable strategy.

### API Clients
API clients implement the functionality of the service. This is clearly application specific, so the
implementation will be left up to the application. There can be high numbers of clients, so using an
async server is recommended. Client requests should be properly translated into messages destined
for processes that perform the service operation and sent along with some sort of correlation id for
the client connection so that responses can be sent back on the proper client connection. The
executor thread will manage a client sender so that responses can be sent back to the client
thread and forwarded onto the client sockets using the correlation id.

### Caveats

##### Expensive Computation inside processes
A major design issue of the current architecture is that all processes are run in a single executor
thread, and processes are executed as soon as an envelope is received that is destined for them.
This means that there is no scheduling at play and some processes will get more CPU time than
other processes. Latency is bound by the processing speed of this thread. Note, however, that since
no IO is performed in these processes that each one will be able to handle a message and finish
without blocking. However, if expensive computations are run in these processes, this architecture
may not be suitable. Latency may be extreme and the queue may get very large. In these cases it may
be better to run those computations in a separate thread and forward the results back to the
appropriate process in a similar manner to how I/O threads work. If this becomes an issue in real
software, this capability will be added.

### 100% CPU use on the executor thread
This case is related to the above, but can occur even in scenarios with very lightweight process
operations. If this occurs while I/O is not saturated and other cores have capacity, it will be
necessary to turn the executor into multiple executor threads. While a simple thread pool is
possible, it's likely that and scheduling fairness will become an issue, with all it's inherent
complexity. This will require a more sophisticated scheduling solution. As stated in the
introduction, this code is being put on hold in order to get the first version of rabble out the
door. There is no question that implementing this will be more complex than the rest of the code
combined, but luckily there is much research on the subject.  While coroutines could certainly be
used in such a scenario, they don't have the benefits of simple actors that can only receive
messages and return messages to be sent, primarily test-ability.  Additionally, coroutines don't
play well with the original architecture as they are allowed to perform I/O.

### 100% CPU use on the cluster server thread
It's quite possible that serialization, deserialization and packet sending and receiving will reach
CPU saturation before the executor thread. In this case a thread pool will be necessary. This does
not have the complexity of process scheduling above since operations are not as fine grained and
fairness is unlikely to be an issue as packets can only be received in the order they are sent. It's
also easy to round robin reading and writing X bytes from and to each socket.

### Back Pressure
In order to simplify the programming model inside cluster, all channels between threads are rust
standard non-blocking asynchronous channels. This provides no back pressure whatsoever. However, this
is not a problem since rust processes can only receive and not generate messages. The only way for a
node to get overloaded is by interaction with external clients. Therefore, all back pressure should
be provided at the boundary between the clients and client thread. A good way to do this is to track
round trip latency of client requests in a sliding window and stop processing requests until the
latency starts to settle down. Since no requests are being processed, and the ones already in the
queue will just have higher latency than those before them, how will we know that the
latency has gone down? Canary messages can be periodically sent and tracked. When the canary latency
has dropped low enough, with hysteresis, we can begin accepting client requests again. Note that
this strategy works especially well in systems where each client can only have one outstanding
request at a time. If clients are allowed to pipeline requests, explicit Nacks would be required.
Other back pressure strategies can be used as well. Since this topic is complex and likely
application specific, it will not be further discussed.
