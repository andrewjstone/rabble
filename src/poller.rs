use std::sync::mpsc::Sender;
use amy::{Poller, Notification};
use rabble_msg::RabbleMsg;
use msg::Msg;


const TIMEOUT: usize = 5000; // ms

/// Infinitely poll for and send messages to the cluster server
pub fn run<T>(tx: Sender<Msg<T>>) {
    let mut poller = Poller::new().unwrap();
    loop {
        let notifications = poller.wait(TIMEOUT).unwrap();
        if let Err(_) = tx.send(Msg::Rabble(RabbleMsg::PollNotifications(notifications))) {
            // The process is exiting
            return;
        }
    }
}
