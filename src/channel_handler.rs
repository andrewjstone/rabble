use rustc_serialize::{Encodable, Decodable};
use service::Service;
use handler::Handler;

pub struct SystemEnvelopeHandler<T: Encodable+Decodable, U> {
    id: usize,
    callback: Box<Fn(SystemEnvelope<U>)>
}

impl<T: Encodable+Decodable, U>  SystemEnvelopeHandler<T, U> {
    pub fn new<F>(callback: F) -> SystemEnvelopeHandler<T, U> where F: Fn(SystemEnvelope<U>) {
        SystemEnvelopeHandler {
            id: 0, // Will be replaced with the correct id when registerd with the service
            callback: callback
        }
    }
}

impl<T: Encodable + Decodable, U> Handler<T, U> for ChannelHandler<T, U> {
    fn register(&mut self, service: &mut Service<T, U>, id: usize) {
        self.id = id;
    }
}
