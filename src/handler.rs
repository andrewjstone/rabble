use rustc_serialize::{Encodable, Decodable};
use service::Service;

// A service handler
pub trait Handler<T: Encodable + Decodable, U> {
    fn register(&mut self, &mut Service<T, U>, id: usize);
    fn ready(&mut self, &mut Service<T, U>);
}
