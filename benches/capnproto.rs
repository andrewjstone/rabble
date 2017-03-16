#![feature(test)]

extern crate test;
extern crate capnp;
extern crate rabble;

use test::Bencher;
use capnp::message::{HeapAllocator, ReaderOptions};
use rabble::{Pid, NodeId};

pub mod msg_capnp {
    include!(concat!(env!("OUT_DIR"), "/msg_capnp.rs"));
}

use msg_capnp::{pid};

pub fn build_pid() -> ::capnp::message::Builder<HeapAllocator> {
    let mut message = ::capnp::message::Builder::new_default();
    {
        let mut pid = message.init_root::<pid::Builder>();
        pid.set_name("pid1");
        pid.set_group("test_group");
        let mut node_id = pid.init_node();
        node_id.set_name("node1");
        node_id.set_addr("127.0.0.1:5000");
    }
    message
}

#[test]
fn serialize_correctly() {
    let pid = build_pid();
    let words = ::capnp::serialize::write_message_to_words(&pid);
    let res = ::capnp::serialize::read_message_from_words(&words, ReaderOptions::new());
    assert!(res.is_ok());
    let reader = res.unwrap();
    let msg = reader.get_root::<pid::Reader>().unwrap();
    assert_eq!("pid1", msg.get_name().unwrap());
    assert_eq!("test_group", msg.get_group().unwrap());
    assert_eq!("node1", msg.get_node().unwrap().get_name().unwrap());
    assert_eq!("127.0.0.1:5000", msg.get_node().unwrap().get_addr().unwrap());
}

#[bench]
fn bench_build_pid(b: &mut Bencher) {
    b.iter(|| build_pid());
}

#[bench]
fn bench_serialize_pid(b: &mut Bencher) {
    let pid = build_pid();
    b.iter(|| ::capnp::serialize::write_message_to_words(&pid));
}

#[bench]
fn bench_deserialize_pid(b: &mut Bencher) {
    let pid = build_pid();
    let words = ::capnp::serialize::write_message_to_words(&pid);
    b.iter(|| ::capnp::serialize::read_message_from_words(&words, ReaderOptions::new()));
}

#[bench]
fn bench_deserialized_pid_to_rust_type(b: &mut Bencher) {
    let pid = build_pid();
    let words = ::capnp::serialize::write_message_to_words(&pid);
    let reader = ::capnp::serialize::read_message_from_words(&words, ReaderOptions::new()).unwrap();
    b.iter(|| {
        let msg = reader.get_root::<pid::Reader>().unwrap();
        let node = msg.get_node().unwrap();
        Pid {
            name: msg.get_name().unwrap().to_string(),
            group: Some(msg.get_group().unwrap().to_string()),
            node: NodeId {
                name: node.get_name().unwrap().to_string(),
                addr: node.get_addr().unwrap().to_string()
            }
        }
    });
}
