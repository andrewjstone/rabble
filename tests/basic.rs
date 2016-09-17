extern crate rabble;
#[macro_use]
extern crate assert_matches;


use std::thread;

use rabble::{
    rouse,
    NodeId,
    Service,
    SystemEnvelopeHandler,
    CorrelationId,
    SystemMsg
};

#[test]
fn single_service_and_handler_get_executor_status() {
    let node_id = NodeId {name: "node1".to_string(), addr: "127.0.0.1:11000".to_string()};
    let (node, handles) = rabble::rouse::<u64, u64>(node_id);
    let mut service = Service::new("test-service", node.clone());
    let pid = service.pid.clone();
    let handler = SystemEnvelopeHandler::new(move |envelope| {
        assert_eq!(envelope.to, pid);
        assert_matches!(envelope.msg, SystemMsg::ExecutorStatus(_));
    });
    let pid = service.pid.clone();
    let handler_id = service.add_handler(Box::new(handler)).unwrap();
    let correlation_id = CorrelationId::handler(handler_id);
    node.executor_status(pid, correlation_id).unwrap();
    thread::spawn(move || {
        service.wait();
    });
}
