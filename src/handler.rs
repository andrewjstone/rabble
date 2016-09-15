use rustc_serialize::{Encodable, Decodable};
use amy::Registrar;
use envelope::SystemEnvelope;

/// Specifies how this handler is to be used by a Service
pub struct HandlerSpec {
    pub default_handler: bool,
    pub requires_poller: bool
}

// A service handler
pub trait Handler<T: Encodable + Decodable, U> {
    fn set_id(&mut self, id: usize);
    fn get_spec(&self) -> HandlerSpec;
    fn register_with_poller(&mut self, &Registrar) -> usize {
        panic!("register_with_poller not implemented, but spec requires_poller")
    }
    fn handle_system_envelope(&mut self, envelope: SystemEnvelope<U>);
}
