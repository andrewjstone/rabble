use std::sync::mpsc::Sender;
use rustc_serialize::{Encodable, Decodable};
use amy::{Poller, Notification};
use internal_msg::InternalMsg;

const TIMEOUT: usize = 5000; // ms

/// Infinitely poll for and send messages to the cluster server
pub fn run<T>(tx: Sender<InternalMsg<T>>) where T: Encodable + Decodable {
    let mut poller = Poller::new().unwrap();
    loop {
        let notifications = poller.wait(TIMEOUT).unwrap();
        if let Err(_) = tx.send(InternalMsg::PollNotifications(notifications)) {
            // The process is exiting
            return;
        }
    }
}
