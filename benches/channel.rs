#![feature(test)]

extern crate test;
extern crate rabble;

use test::Bencher;
use rabble::{Pid, NodeId, Envelope, Msg, CorrelationId};

use std::sync::mpsc::channel;

trait MsgTrait {}

pub fn pid() -> Pid {
    Pid {
        name: "pid1".to_string(),
        group: Some("rabble".to_string()),
        node: NodeId {
            name: "node1".to_string(),
            addr: "127.0.0.1:5000".to_string()
        }
    }
}

impl MsgTrait for Envelope<u64> {}
impl MsgTrait for Vec<u64> {}

pub fn envelope() -> Envelope<u64> {
    Envelope {
        to: pid(),
        from: pid(),
        correlation_id: Some(CorrelationId::pid(pid())),
        msg: Msg::Pids((0..5).map(|_| pid()).collect())
    }
}

#[bench]
fn build_and_send_unboxed(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        let envelope = envelope();
        tx.send(envelope)
    });
}

#[bench]
fn build_and_send_trait_object(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        let envelope = envelope();
        tx.send(Box::new(envelope) as Box<MsgTrait>)
    });
}

#[bench]
fn build_and_send_4_ints(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        tx.send([7u64,8u64,9u64,10u64])
    });
}

#[bench]
fn build_and_send_16_ints(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        tx.send([0u64,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15])
    });
}

#[bench]
fn build_and_send_vec_of_16_ints(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        tx.send(vec![0u64,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15])
    });
}

#[bench]
fn build_and_send_vec_of_16_ints_as_trait_object(b: &mut Bencher) {
    let (tx, rx) = channel();
    b.iter(|| {
        tx.send(Box::new(vec![0u64,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]) as Box<MsgTrait>)
    });
}
