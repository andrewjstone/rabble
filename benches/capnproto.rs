#![feature(test)]

extern crate test;
extern crate capnp;
extern crate rabble;

use test::Bencher;
use capnp::message::{HeapAllocator, ReaderOptions};
use rabble::{Pid, NodeId, CorrelationId, Envelope, Msg};

pub mod msg_capnp {
    include!(concat!(env!("OUT_DIR"), "/msg_capnp.rs"));
}

use msg_capnp::{pid, envelope};
use msg_capnp::msg::Which::Reply;
use msg_capnp::reply::Which::Processes;

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

pub fn build_envelope_of_processes() -> ::capnp::message::Builder<HeapAllocator> {
    let mut message = ::capnp::message::Builder::new_default();
    {
        let mut envelope = message.init_root::<envelope::Builder>();

        {
            let mut to = envelope.borrow().init_to();
            to.set_name("to_pid");
            to.set_group("test_group");
            let mut node_id = to.init_node();
            node_id.set_name("node1");
            node_id.set_addr("127.0.0.1:5000");
        }

        {
            let mut from = envelope.borrow().init_from();
            from.set_name("from_pid");
            from.set_group("test_group");
            let mut node_id = from.init_node();
            node_id.set_name("node2");
            node_id.set_addr("127.0.0.1:5000");
        }

        {
            let mut correlationid = envelope.borrow().init_cid();
            {
                let mut pid = correlationid.borrow().init_pid();
                pid.set_name("from_pid");
                pid.set_group("test_group");
                let mut node_id = pid.init_node();
                node_id.set_name("node2");
                node_id.set_addr("127.0.0.1:5000");
            }
            correlationid.set_handle(0);
            correlationid.set_request(1);
        }

        let msg = envelope.init_msg();
        let reply = msg.init_reply();
        let mut pids = reply.init_processes(3);
        for i in 0..3 {
            let mut pid = pids.borrow().get(i);
            // Don't bother using diff names, cause we don't want formatting in the benchmark
            pid.set_name("pidx");
            pid.set_group("test_group");
            let mut node_id = pid.init_node();
            node_id.set_name("nodex");
            node_id.set_addr("some_ip_addr_and_port");
        }
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
    b.iter(|| get_pid(reader.get_root::<pid::Reader>().unwrap()));
}

#[bench]
fn bench_build_envelope_of_processes(b: &mut Bencher) {
    b.iter(|| build_envelope_of_processes());
}

#[bench]
fn bench_serialize_envelope_of_processes(b: &mut Bencher) {
    let envelope = build_envelope_of_processes();
    b.iter(|| capnp::serialize::write_message_to_words(&envelope));
}

#[bench]
fn bench_deserialize_envelope_of_processes(b: &mut Bencher) {
    let envelope = build_envelope_of_processes();
    let words = ::capnp::serialize::write_message_to_words(&envelope);
    b.iter(|| ::capnp::serialize::read_message_from_words(&words, ReaderOptions::new()));
}

#[bench]
fn bench_deserialized_envelope_of_processes_to_rust_type(b: &mut Bencher) {
    let envelope = build_envelope_of_processes();
    let words = ::capnp::serialize::write_message_to_words(&envelope);
    let reader = ::capnp::serialize::read_message_from_words(&words, ReaderOptions::new()).unwrap();
    b.iter(|| {
        let envelope = reader.get_root::<envelope::Reader>().unwrap();
        let to = get_pid(envelope.get_to().unwrap());
        let from = get_pid(envelope.get_from().unwrap());
        let cid = {
            let cid = envelope.get_cid().unwrap();
            let pid = get_pid(cid.get_pid().unwrap());
            CorrelationId {
                pid: pid,
                connection: Some(cid.get_handle()),
                request: Some(cid.get_request())
            }
        };
        let msg = envelope.get_msg().unwrap();
        let pids =  if let Ok(Reply(reply)) = msg.which() {
            if let Ok(Processes(processes)) = reply.unwrap().which() {
                processes.unwrap().iter().map(|p| {
                    get_pid(p)
                }).collect()
            } else {
                unreachable!()
            }
        } else {
            unreachable!()
        };
        Envelope::<u64> {
            to: to,
            from: from,
            correlation_id: Some(cid),
            msg: Msg::Pids(pids)
        }
    });
}

fn get_pid(reader: pid::Reader) -> Pid {
    let node = reader.get_node().unwrap();
    Pid {
        name: reader.get_name().unwrap().to_string(),
        group: Some(reader.get_group().unwrap().to_string()),
        node: NodeId {
            name: node.get_name().unwrap().to_string(),
            addr: node.get_addr().unwrap().to_string()
        }
    }
}
