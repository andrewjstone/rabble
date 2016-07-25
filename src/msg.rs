use rabble_msg::RabbleMsg;

/// The top-level type of messages sent over channels in Rabble.
///
/// This message must contain both rabble internal data types and user defined data types. Therefore
/// an enum is used for this purpose.
pub enum Msg<T> {
    Rabble(RabbleMsg),
    User(T)
}
