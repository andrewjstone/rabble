#![feature(test)]

extern crate test;
extern crate protobuf;
extern crate rabble;

use protobuf::Message;
use test::Bencher;
use rabble::{Pid, NodeId};

#[path = "../schema/msg.rs"]
mod msg;

fn pid() -> Pid {
    Pid {
        name: "pid1".to_string(),
        group: Some("test_group".to_string()),
        node: NodeId {
            name: "node1".to_string(),
            addr: "127.0.0.1:5000".to_string()
        }
    }
}

/// Create a protobuf pid
pub fn build_pid(pid: Pid) -> msg::Pid {
    let mut api_pid = msg::Pid::new();
    api_pid.set_name(pid.name);
    api_pid.set_group(pid.group.unwrap());
    let mut node_id = msg::NodeId::new();
    node_id.set_name(pid.node.name);
    node_id.set_addr(pid.node.addr);
    api_pid
}

#[bench]
fn bench_build_pid(b: &mut Bencher) {
    b.iter(|| build_pid(pid()));
}

#[bench]
fn bench_serialize_pid(b: &mut Bencher) {
    let pid = build_pid(pid());
    b.iter(|| pid.write_to_bytes());
}

#[bench]
fn bench_deserialize_pid(b: &mut Bencher) {
    let pid = build_pid(pid());
    let serialized = pid.write_to_bytes().unwrap();
    b.iter(|| protobuf::parse_from_bytes::<msg::Pid>(&serialized));
}

#[bench]
fn bench_deserialize_pid_to_rust_type(b: &mut Bencher) {
    let pid = build_pid(pid());
    let serialized = pid.write_to_bytes().unwrap();
    let mut deserialized = protobuf::parse_from_bytes::<msg::Pid>(&serialized).unwrap();

    b.iter(|| {
        let mut node_id = deserialized.take_node();
        Pid {
            name: deserialized.take_name(),
            group: Some(deserialized.take_group()),
            node: NodeId {
                name: node_id.take_name(),
                addr: node_id.take_addr()
            }
        }
    });
}
