use std::marker::PhantomData;
use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use handler::Handler;
use envelope::SystemEnvelope;
use node::Node;
use errors::*;

pub struct SystemEnvelopeHandler<T: Encodable+Decodable, U: Debug + Clone> {
    callback: Box<Fn(SystemEnvelope<U>) + Send>,
    unused: PhantomData<T>
}

impl<T, U> SystemEnvelopeHandler<T, U> where T: Encodable + Decodable, U: Debug + Clone {
    pub fn new<F>(callback: F) -> SystemEnvelopeHandler<T, U>
      where F: Fn(SystemEnvelope<U>) + 'static + Send {
          SystemEnvelopeHandler {
              callback: Box::new(callback),
              unused: PhantomData
          }
    }
}

impl<T, U> Handler<T, U> for SystemEnvelopeHandler<T, U>
    where T: Encodable + Decodable, U: Debug + Clone
{
    fn handle_system_envelope(&mut self,
                              node: &Node<T, U>,
                              envelope: SystemEnvelope<U>) -> Result<()>
    {
        (self.callback)(envelope);
        Ok(())
    }
}
