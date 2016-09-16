use std::marker::PhantomData;
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use service::Service;
use handler::{Handler, HandlerSpec};
use envelope::SystemEnvelope;
use node::Node;

pub struct SystemEnvelopeHandler<T: Encodable+Decodable, U: Debug + Clone> {
    id: usize,
    callback: Box<Fn(SystemEnvelope<U>)>,
    unused: PhantomData<T>
}

impl<T, U> SystemEnvelopeHandler<T, U> where T: Encodable + Decodable, U: Debug + Clone {
    pub fn new<F>(callback: F) -> SystemEnvelopeHandler<T, U>
      where F: Fn(SystemEnvelope<U>) + 'static {
          SystemEnvelopeHandler {
              id: 0, // Will be replaced with the correct id when registerd with the service
              callback: Box::new(callback),
              unused: PhantomData
          }
    }
}

impl<T, U> Handler<T, U> for SystemEnvelopeHandler<T, U>
  where T: Encodable + Decodable, U: Debug + Clone {

    fn set_id(&mut self, id: usize) {
        self.id = id
    }

    fn get_spec(&self) -> HandlerSpec {
        HandlerSpec {
            default_handler: true,
            requires_poller: false
        }
    }

    fn handle_system_envelope(&mut self, node: &Node<T, U>, envelope: SystemEnvelope<U>) {
        let ref f = self.callback;
        f(envelope);
    }
}
