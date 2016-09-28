use std::io;
use std::sync::mpsc;
use pid::Pid;

/// Used by the error-chain crate to generate errors
error_chain! {
    foreign_links {
        io::Error, Io;
    }

    errors {
        SendError(msg: String) {
            description("failed to send")
            display("failed to send {}", msg)
        }
        Shutdown(pid: Pid) {
            description("shutting down")
            display("shutting down {}", pid)
        }
    }
}
