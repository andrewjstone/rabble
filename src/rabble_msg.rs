use amy::Notification;

/// A message type used internally in Rabble
pub enum RabbleMsg {
   PollNotifications(Vec<Notification>),
}
