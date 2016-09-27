use pid::Pid;

/// Match requests through the system with their handlers
///
/// All correlation ids must have a pid.
/// Sometimes individual connections/requests aren't tracked so that field is optional.
#[derive(Debug, Hash, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct CorrelationId {
    pub pid: Pid,
    pub connection: Option<usize>,
    pub request: Option<usize>
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
    pub fn connection(pid: Pid, connection_id: usize) -> CorrelationId {
        CorrelationId {
            pid: pid,
            connection: Some(connection_id),
            request: None
        }
    }

    /// Create a correlation id that matches a handler, connection, and request
    pub fn request(pid: Pid, connection_id: usize, request_id: usize) -> CorrelationId {
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
