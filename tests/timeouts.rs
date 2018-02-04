extern crate rabble;

use std::time::Duration;
use std::sync::mpsc;
use std::thread::JoinHandle;

extern crate serde;
#[macro_use]
extern crate serde_derive;

use rabble::{
    Node,
    Msg,
    Pid,
    Process,
    NodeId,
    Envelope,
    Terminal,
    TimerId,
    channel
};

fn setup(addr: &str) -> (Node<TestMsg>,
                         Pid,
                         Pid,
                         mpsc::Receiver<Envelope<TestMsg>>,
                         Vec<JoinHandle<()>>)
{
    let node_id = NodeId {name: "node1".to_string(), addr: addr.to_string()};
    let (node, handles) = rabble::rouse::<TestMsg>(node_id.clone(), None);

    let pid = Pid {
        name: "some-process".to_string(),
        group: None,
        node: node_id
    };

	let mut test_pid = pid.clone();
    test_pid.name = "test_pid".to_string();

    let (tx, rx) = mpsc::channel();

    let process = TestProcess {
        test_pid: test_pid.clone(),
		timer_id: TimerId::new(0)
    };

    node.spawn(&pid, Box::new(process)).unwrap();
    node.register_service(&test_pid,
                          Box::new(tx) as Box<channel::Sender<Envelope<TestMsg>>>).unwrap();

    (node, pid, test_pid, rx, handles)
}

#[test]
fn timer_fires() {
    let (node, pid, test_pid, rx, handles) = setup("127.0.0.1:11003");
    node.send(Envelope::new(pid.clone(), test_pid.clone(), Msg::User(TestMsg::StartTimer))).unwrap();

    // Wait for the process to get the timeout. Wait 3x as long as the actual timeout.
    let envelope = rx.recv_timeout(Duration::from_millis(300))
        .expect("Timeout not received by process!");
    assert_eq!(envelope.msg, Msg::User(TestMsg::TimerFired));
    assert_eq!(envelope.from, pid);
    assert_eq!(envelope.to, test_pid);

    node.shutdown();
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn cancelled_timer_does_not_fire() {
    let (node, pid, test_pid, rx, handles) = setup("127.0.0.1:11004");
    node.send(Envelope::new(pid.clone(), test_pid.clone(), Msg::User(TestMsg::StartTimer))).unwrap();
    node.send(Envelope::new(pid.clone(), test_pid.clone(), Msg::User(TestMsg::CancelTimer))).unwrap();

    // Wait for 3x the timeout length. The process should not have received the timeout.
    match rx.recv_timeout(Duration::from_millis(300)) {
        Ok(_) => panic!("Timeout received when it should have been cancelled"),
        Err(_)  => ()
    }

    node.shutdown();
    for h in handles {
        h.join().unwrap();
    }
}


#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
enum TestMsg {
	StartTimer,
	CancelTimer,
    TimerFired
}

struct TestProcess {
    test_pid: Pid,
    timer_id: TimerId
}

impl<T> Process<TestMsg, T> for TestProcess where T: Terminal<TestMsg> {
    fn handle(&mut self,
              msg: Msg<TestMsg>,
              _: Pid,
              terminal: &mut T) {

        match msg {
            Msg::Timeout(timer_id) => {
				assert_eq!(timer_id, self.timer_id);
				// Alert the test runner that the timeout was received
				terminal.send(self.test_pid.clone(), TestMsg::TimerFired);
			}
            Msg::User(TestMsg::StartTimer) => {
				self.timer_id = terminal.start_timer(Duration::from_millis(100));
			}
            Msg::User(TestMsg::CancelTimer) => {
                terminal.cancel_timer(self.timer_id);
			}
            _ => unreachable!()
		}
    }
}
