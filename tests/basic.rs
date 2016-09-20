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
    SystemMsg,
    Pid
};

#[test]
fn single_service_and_handler_get_executor_status() {
    let node_id = NodeId {name: "node1".to_string(), addr: "127.0.0.1:11000".to_string()};
    let (node, handles) = rabble::rouse::<u64, u64>(node_id);
    let pid = Pid {
        name: "test-service".to_string(),
        group: Some("Service".to_string()),
        node: node.id.clone()
    };
    let pid2 = pid.clone();
    let handler = SystemEnvelopeHandler::new(move |envelope| {
        assert_eq!(envelope.to, pid);
        assert_matches!(envelope.msg, SystemMsg::ExecutorStatus(_));
    });
    let mut service = Service::new(pid2.clone(), node.clone(), handler);
    node.executor_status(pid2, None).unwrap();
    thread::spawn(move || {
        service.wait();
    });
}
