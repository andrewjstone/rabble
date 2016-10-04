use std::fmt::Debug;
use rustc_serialize::{Encodable, Decodable};
use service_handler::ServiceHandler;
use envelope::Envelope;
use node::Node;
use errors::*;
use amy::Registrar;

pub struct ThreadHandler<T: Encodable + Decodable + Debug + Clone> {
    callback: Box<Fn(Envelope<T>) + Send>
}

impl<T> ThreadHandler<T> where T: Encodable + Decodable + Debug + Clone {
    pub fn new<F>(callback: F) -> ThreadHandler<T>
      where F: Fn(Envelope<T>) + 'static + Send {
          ThreadHandler {
              callback: Box::new(callback),
          }
    }
}

impl<T> ServiceHandler<T> for ThreadHandler<T>
    where T: Encodable + Decodable + Debug + Clone
{
    fn handle_envelope(&mut self,
                       _node: &Node<T>,
                       envelope: Envelope<T>,
                       _: &Registrar) -> Result<()>
    {
        (self.callback)(envelope);
        Ok(())
    }
}
