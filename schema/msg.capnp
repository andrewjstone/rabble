@0xc7f966af6a869c92;

struct Pid {
    # A process id
    
    name @0 :Text;
    node @1 :NodeId;
    group @2 :Text;
}

struct Envelope {
    # All rabble messages are contained in envelopes

    to @0 :Pid;
    from @1 :Pid;
    msg @2 :Msg;
    cid @3 :CorrelationId;
}

struct CorrelationId {
    pid @0 :Pid;
    handle @1 :UInt64; # This can be used for things like connection ids
    request @2 :UInt64;  # An optional monotonic request count
}

struct NodeId {
    name @0 :Text;
    addr @1 :Text; # IP address and port formatted as "127.0.0.1:5000"
}

struct ClusterMsg {
    union {
        members @0 :MemberSet;
	ping @1 :Void;
	envelope @2 :Envelope;
	delta @3 :Data;
    }
}

struct MemberSet {
    # A message used by the cluster server protocol to join nodes. This is not user visible.

    from @0 :NodeId;
    orset @1 :Data;
}

struct Msg(UserReq, UserRpy) {
    # The main rabble message type

    union {
	userRequest @0 :UserReq; # The request type defined by users of rabble
	userReply @1 :UserRpy; # The reply type defined by users of rabble
	request @2 :Request; 
	reply @3 :Reply; 
    }
}

struct Request {
    union {
	getMetrics @0 :Void;
	startTimer @1 :UInt64; # timeout in ms
	cancelTimer @2 :Void; # The correlation_id for the timer is in the envelope
	shutdown @3 :Void;
	getProcesses @4 :NodeId;
	getServices @5 :NodeId;
    }
}

struct Reply {
    union {
	metrics @0 :List(Metric);
	timeout @1 :Void; # The correlation_id for the timer is in the envelope
	processes @2 :List(Pid); # All processes running on a given node
	services @3 :List(Pid); # All services running on a given node
	members @4 :List(Member);
    }
}

struct Member {
    node @0 :NodeId;
    connected @1 :Bool;
}

struct Metric {
    union {
        gauge @0 :Int64;
	counter @1 :UInt64;
	serializedHistogram @2 :Data;
    }
}
