#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Msg<T> {
    User(T),
    Timeout,
    Shutdown
}
