#![feature(test)]

extern crate test;
extern crate rabble;

use test::Bencher;
use rabble::{Pid, NodeId};

pub fn pid1() -> Pid {
    Pid {
        name: "pid1".to_string(),
        group: Some("rabble".to_string()),
        node: NodeId {
            name: "node1".to_string(),
            addr: "127.0.0.1:5000".to_string()
        }
    }
}

pub fn pid2() -> Pid {
    Pid {
        name: "pid2".to_string(),
        group: Some("rabble".to_string()),
        node: NodeId {
            name: "node1".to_string(),
            addr: "127.0.0.1:6000".to_string()
        }
    }
}

#[bench]
fn construction(b: &mut Bencher) {
    b.iter(|| pid1());
}

#[bench]
fn construction_and_allocation(b: &mut Bencher) {
    b.iter(|| Box::new(pid1()));
}

#[bench]
fn equality_when_different(b: &mut Bencher) {
    let pid1 = pid1();
    let pid2 = pid1.clone();
    b.iter(|| pid1 == pid2);
}

#[bench]
fn inequality_when_different(b: &mut Bencher) {
    let pid1 = pid1();
    let pid2 = pid2();
    b.iter(|| pid1 != pid2);
}

#[bench]
fn u64_equality(b: &mut Bencher) {
    b.iter(|| 1 == 9999999);
}

#[bench]
fn u64_inequality(b: &mut Bencher) {
    b.iter(|| 1 != 9999999);
}
