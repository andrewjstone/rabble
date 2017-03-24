use std::convert::From;
use pid::Pid;
use pb_messages;

/// Match requests through the system with their handlers
///
/// All correlation ids must have a pid.
/// Sometimes individual connections/requests aren't tracked so that field is optional.
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct CorrelationId {
    pub pid: Pid,
    pub connection: Option<u64>,
    pub request: Option<u64>
}

impl CorrelationId {

    pub fn pid(pid: Pid) -> CorrelationId {
        CorrelationId {
            pid: pid,
            connection: None,
            request: None,
        }
    }

    /// Create a correlation id that matches a handler and connection
    pub fn connection(pid: Pid, connection_id: u64) -> CorrelationId {
        CorrelationId {
            pid: pid,
            connection: Some(connection_id),
            request: None
        }
    }

    /// Create a correlation id that matches a handler, connection, and request
    pub fn request(pid: Pid, connection_id: u64, request_id: u64) -> CorrelationId {
        CorrelationId {
            pid: pid,
            connection: Some(connection_id),
            request: Some(request_id)
        }
    }

    /// Clone the CorrelationId and increment the request counter
    pub fn next_request(&self) -> CorrelationId {
        let mut id = self.clone();
        id.request = id.request.map(|req| req + 1);
        id
    }
}

impl From<pb_messages::CorrelationId> for CorrelationId {
    fn from(mut pb_cid: pb_messages::CorrelationId) -> CorrelationId {
        let connection = if pb_cid.has_handle() {
            Some(pb_cid.get_handle())
        } else {
            None
        };
        let request = if pb_cid.has_request() {
            Some(pb_cid.get_request())
        } else {
            None
        };
        CorrelationId {
            pid: pb_cid.take_pid().into(),
            connection: connection,
            request: request
        }
    }
}

impl From<CorrelationId> for pb_messages::CorrelationId {
    fn from(cid: CorrelationId) -> pb_messages::CorrelationId {
        let mut pb_cid = pb_messages::CorrelationId::new();
        pb_cid.set_pid(cid.pid.into());
        if let Some(connection) = cid.connection {
            pb_cid.set_handle(connection);
        }
        if let Some(request) = cid.request {
            pb_cid.set_request(request);
        }
        pb_cid
    }
}
