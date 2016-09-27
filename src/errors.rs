use std::io;
use std::sync::mpsc;
use pid::Pid;

/// Used by the error-chain crate to generate errors
error_chain! {
    foreign_links {
        io::Error, Io;
    }

    errors {
        Shutdown(pid: Pid) {
            description("shutting down")
            display("shutting down {}", pid)
        }
    }
}
