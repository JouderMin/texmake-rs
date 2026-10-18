//! Error types shared by every texmake subcommand.
//!
//! Every error knows its process exit code: usage/environment problems exit
//! with 2, build failures with 1. Later tasks extend the enum with the config
//! and build variants; `exit_code` stays the single mapping point.

use std::fmt;

/// A texmake failure, mapped to a process exit code by [`Error::exit_code`].
#[derive(Debug)]
pub enum Error {
    /// Invalid command-line usage.
    Usage(String),
    /// A problem with the environment texmake runs in.
    // Constructed once config discovery lands (Task 2); keeping it here fixes
    // the exit-code contract for later tasks.
    #[allow(dead_code)]
    Environment(String),
}

impl Error {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Environment(_) => 2,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(message) | Error::Environment(message) => {
                write!(f, "error: {message}")
            }
        }
    }
}

impl std::error::Error for Error {}
