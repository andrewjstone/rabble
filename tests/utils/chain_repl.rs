use std::sync::mpsc::Receiver;
use super::messages::TestMsg;

use rabble::{
    Pid,
    Msg,
    Envelope,
    Node
};

/// Send 3 operations to the head of the chain
#[allow(dead_code)] // Not used in all tests
pub fn run_client_operations(node: &Node<TestMsg>,
                             head: &Pid,
                             test_pid: &Pid,
                             rx: &Receiver<Envelope<TestMsg>>)
{
    // Pipeline 3 message requests
    for i in 0..3 {
        let msg = Msg::User(TestMsg::Op(i));
        node.send(Envelope::new(head.clone(), test_pid.clone(), msg)).unwrap();
    }

    // Try to receive all 3 messages
    for _ in 0..3 {
        match rx.recv() {
            Ok(envelope) => assert_eq!(envelope.msg, Msg::User(TestMsg::OpComplete)),
            e => {
                println!("Failed to receive OpComplete. Got {:?}", e);
                assert!(false);
            }
        }
    }
}

/// Verify that after all client operations have gotten replies that the history of operations in
/// each replica is identical.
#[allow(dead_code)] // Not used in all tests
pub fn verify_histories(node: &Node<TestMsg>,
                        pids: &Vec<Pid>,
                        test_pid: &Pid,
                        rx: &Receiver<Envelope<TestMsg>>)
{
    let mut history = Vec::new();
    for (i, pid) in pids.clone().into_iter().enumerate() {
        let msg = Msg::User(TestMsg::GetHistory);
        node.send(Envelope::new(pid, test_pid.clone(), msg)).unwrap();
        match rx.recv() {
            Ok(Envelope{msg: Msg::User(TestMsg::History(h)), ..}) => {
                assert!(h.len() != 0);
                if i == 0 {
                    history = h;
                } else {
                    assert_eq!(history, h);
                }
            }
            e => {
                println!("Failed to receive history. Got {:?}", e);
                assert!(false);
            }
        }
    }
}
