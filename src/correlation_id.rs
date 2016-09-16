/// Match requests through the system with their handlers
///
/// All correlation ids must have a handler.
/// Some handlers don't track individual connections or requests, so those fields are optional.
///

#[derive(Debug, Clone, Eq, PartialEq, RustcEncodable, RustcDecodable)]
pub struct CorrelationId {
    pub handler: usize,
    pub connection: Option<usize>,
    pub request: Option<usize>
}

impl CorrelationId {
    /// Create a correlation id that only matches a specific handler
    pub fn handler(handler_id: usize) -> CorrelationId {
        CorrelationId {
            handler: handler_id,
            connection: None,
            request: None
        }
    }

    /// Create a correlation id that matches a handler and connection
    pub fn connection(handler_id: usize, connection_id: usize) -> CorrelationId {
        CorrelationId {
            handler: handler_id,
            connection: Some(connection_id),
            request: None
        }
    }

    /// Create a correlation id that matches a handler, connection, and request
    pub fn request(handler_id: usize, connection_id: usize, request_id: usize) -> CorrelationId {
        CorrelationId {
            handler: handler_id,
            connection: Some(connection_id),
            request: None
        }
    }

    /// Clone the CorrelationId and increment the request counter
    pub fn next_request(&self) -> CorrelationId {
        let mut id = self.clone();
        id.request = id.request.map(|req| req + 1);
        id
    }
}
