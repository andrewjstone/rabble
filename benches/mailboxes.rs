#![feature(test)]

extern crate test;
extern crate rabble;

use test::Bencher;
use std::sync::mpsc::channel;
use std::thread::spawn;
use rabble::{Pid, Envelope, CorrelationId, NodeId, Mailboxes};

fn pid(name: &str) -> Pid {
    Pid {
        group: None,
        name: name.to_owned(),
        node: node_id()
    }
}

fn node_id() -> NodeId {
    NodeId {
        name: "bencher".to_owned(),
        addr: "127.0.0.1:5000".to_owned()
    }
}

#[bench]
fn send_message(b: &mut Bencher) {
    let mailboxes = Mailboxes::new();
    let to = pid("pid1");
    let from = pid("bencher");
    mailboxes.create_mailbox(to.clone()).unwrap();
    let envelope = Envelope::new(to, from, rabble::Msg::User(()), None);
    b.iter(|| mailboxes.send(envelope.clone()))
}

#[bench]
fn send_1000_messages(b: &mut Bencher) {
    let mailboxes = Mailboxes::new();
    let to = pid("pid1");
    let from = pid("bencher");
    mailboxes.create_mailbox(to.clone()).unwrap();
    let envelope = Envelope::new(to, from, rabble::Msg::User(()), None);
    b.iter(|| {
        for _ in 0..1000 {
            mailboxes.send(envelope.clone());
        }
    })
}

#[bench]
/*fn send_1000_messages_2_threads(b: &mut Bencher) {
    let mailboxes = Mailboxes::new();
    let to = pid("pid1");
    let from = pid("bencher");
    mailboxes.create_mailbox(to.clone()).unwrap();
    let envelope = Envelope::new(to, from, rabble::Msg::User(()), None);
    let envelope2 = envelope.clone();
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    let (tx_done, rx_done) = channel();
    let tx_done2 = tx_done.clone();
    let mailboxes2 = mailboxes.clone();
    spawn(move || {
        /// Start sending
        let _ = rx1.recv().unwrap();
        for _ in 0..500 {
            mailboxes.send(envelope.clone());
        }
        tx_done.send(());
    });
    spawn(move || {
        /// Start sending
        let _ = rx2.recv().unwrap();
        for _ in 0..500 {
            mailboxes2.send(envelope2.clone());
        }
        tx_done2.send(());
    });

    // This includes channel sending and thread park/unpark so not sure how accurate it actually is
    // for what we want to measure.;
    b.iter(|| {
        tx1.send(()).unwrap();
        tx2.send(()).unwrap();
        println!("started");
        rx_done.recv().unwrap();
        rx_done.recv().unwrap();
    })
}
*/
