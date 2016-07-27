use rustc_serialize::{Encodable, Decodable};
use amy::Notification;
use node::Node;

/// The top-level type of messages sent over channels in Rabble.
///
/// This message must contain both rabble internal data types and a user defined data type.
/// Note that the user defined data type must be Encodable and Decodable because it can be sent
/// between nodes.
pub enum InternalMsg<T: Encodable + Decodable> {
    PollNotifications(Vec<Notification>),
    Join(Node),
    User(T)
}
