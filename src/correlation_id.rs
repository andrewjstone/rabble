/// Match requests through the system with their handlers
///
/// All correlation ids must have a connection.
/// Sometimes individual requests aren't tracked so that field is optional.

#[derive(Debug, Hash, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct CorrelationId {
    pub connection: usize,
    pub request: Option<usize>
}

impl CorrelationId {

    /// Create a correlation id that matches a handler and connection
    pub fn connection(connection_id: usize) -> CorrelationId {
        CorrelationId {
            connection: connection_id,
            request: None
        }
    }

    /// Create a correlation id that matches a handler, connection, and request
    pub fn request(connection_id: usize, request_id: usize) -> CorrelationId {
        CorrelationId {
            connection: connection_id,
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
