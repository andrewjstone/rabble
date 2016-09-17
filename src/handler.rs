use std::fmt::Debug;
use std::marker::Send;
use rustc_serialize::{Encodable, Decodable};
use amy::{Notification, Registrar};
use envelope::SystemEnvelope;
use node::Node;

/// Specifies how this handler is to be used by a Service
pub struct HandlerSpec {
    pub default_handler: bool,
    pub requires_poller: bool
}

// A service handler
pub trait Handler<T: Encodable + Decodable, U: Debug + Clone> {
    fn set_id(&mut self, id: usize);
    fn get_spec(&self) -> HandlerSpec;
    fn register_with_poller(&mut self, &Registrar) -> Vec<usize> {
        panic!("register_with_poller not implemented, but spec has requires_poller")
    }
    fn handle_notification(&mut self, &Node<T, U>, Notification, &Registrar) {
        panic!("handle_notification not implemented, but spec has requires_poller")
    }
    fn handle_system_envelope(&mut self, &Node<T, U>, SystemEnvelope<U>);
}
