use std::io;

/// Used by the error-chain crate to generate errors
error_chain! {
    foreign_links {
        io::Error, Io;
    }
}
