#![feature(test)]

extern crate test;
extern crate protobuf;
extern crate rabble;

use protobuf::Message;
use test::Bencher;
use rabble::{Pid, NodeId, CorrelationId, Msg, Envelope};

#[path = "../schema/msg.rs"]
mod msg;

/// Create a protobuf pid
pub fn build_pid() -> msg::Pid {
    let mut api_pid = msg::Pid::new();
    api_pid.set_name("pid1".to_string());
    api_pid.set_group("test_group".to_string());
    let mut node_id = msg::NodeId::new();
    node_id.set_name("node1".to_string());
    node_id.set_addr("127.0.0.1:5000".to_string());
    api_pid.set_node(node_id);
    api_pid
}

pub fn build_envelope_of_processes() -> msg::Envelope {
    let mut envelope = msg::Envelope::new();
    envelope.set_to(build_pid());
    envelope.set_from(build_pid());
    let mut cid = msg::CorrelationId::new();
    cid.set_pid(build_pid());
    cid.set_handle(1);
    cid.set_request(1);
    envelope.set_cid(cid);

    let mut pids = msg::Pids::new();
    pids.set_pids((0..3).map(|_| build_pid()).collect());
    let mut msg = msg::Msg::new();
    msg.set_processes(pids);
    envelope.set_msg(msg);
    envelope
}

#[bench]
fn bench_build_pid(b: &mut Bencher) {
    b.iter(|| build_pid());
}

#[bench]
fn bench_serialize_pid(b: &mut Bencher) {
    let pid = build_pid();
    b.iter(|| pid.write_to_bytes());
}

#[bench]
fn bench_deserialize_pid(b: &mut Bencher) {
    let pid = build_pid();
    let serialized = pid.write_to_bytes().unwrap();
    b.iter(|| protobuf::parse_from_bytes::<msg::Pid>(&serialized));
}

#[bench]
fn bench_deserialize_pid_to_rust_type(b: &mut Bencher) {
    let pid = build_pid();
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

#[bench]
fn bench_build_envelope_of_processes(b: &mut Bencher) {
    b.iter(|| build_envelope_of_processes());
}

#[bench]
fn bench_serialize_envelope_of_processes(b: &mut Bencher) {
    let envelope = build_envelope_of_processes();
    b.iter(|| envelope.write_to_bytes());
}

#[bench]
fn bench_deserialize_envelope_of_processes(b: &mut Bencher) {
    let bytes = build_envelope_of_processes().write_to_bytes().unwrap();
    b.iter(|| protobuf::parse_from_bytes::<msg::Envelope>(&bytes));
}

#[bench]
fn bench_deserialized_envelope_of_processes_to_rust_type(b: &mut Bencher) {
    let bytes = build_envelope_of_processes().write_to_bytes().unwrap();
    let mut deserialized = protobuf::parse_from_bytes::<msg::Envelope>(&bytes).unwrap();
    b.iter(|| {
        let to = get_pid(deserialized.take_to());
        let from = get_pid(deserialized.take_from());
        let mut cid = deserialized.take_cid();
        let cid = CorrelationId {
            pid: get_pid(cid.take_pid()),
            connection: Some(cid.get_handle()),
            request: Some(cid.get_request())
        };
        let mut msg = deserialized.take_msg();
        let pids = msg.take_processes().take_pids().into_iter().map(|p| get_pid(p)).collect();
        Envelope::<u64> {
            to: to,
            from: from,
            correlation_id: Some(cid),
            msg: Msg::Pids(pids)
        }
    });

}

fn get_pid(mut deserialized: msg::Pid) -> Pid {
    let mut node_id = deserialized.take_node();
    Pid {
        name: deserialized.take_name(),
        group: Some(deserialized.take_group()),
        node: NodeId {
            name: node_id.take_name(),
            addr: node_id.take_addr()
        }
    }
}
