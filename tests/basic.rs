extern crate rabble;

use rabble::{
    rouse,
    NodeId,
    Service
};

#[test]
fn basic() {
    let node_id = NodeId {name: "node1".to_string(), addr: "127.0.0.1:11000".to_string()};
    let (node, handles) = rabble::rouse::<u64, u64>(node_id);
    let service = Service::new("test-service", node.clone());
}
