extern crate time;

pub mod replica;
pub mod api_server;
pub mod messages;

use std::thread;

use utils::time::{SteadyTime, Duration};

/// Wait for a function to return true
///
/// After each call of `f()` that returns `false`, sleep for `sleep_time`
/// Returns true if `f()` returns true before the timeout expires
/// Returns false if the runtime of the test exceeds `timeout`
#[allow(dead_code)]
pub fn wait_for<F>(sleep_time: Duration, timeout: Duration, mut f: F) -> bool
    where F: FnMut() -> bool
{
    let start = SteadyTime::now();
    while let false = f() {
        thread::sleep(sleep_time.to_std().unwrap());
        if SteadyTime::now() - start > timeout {
            return false;
        }
    }
    true
}
