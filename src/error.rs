//! Error types shared by every texmake subcommand.
//!
//! Every error knows its process exit code: usage/environment/config problems
//! exit with 2, build failures with 1. Later tasks extend the enum with the
//! build variants; `exit_code` stays the single mapping point.

use std::fmt;
use std::path::PathBuf;

/// A texmake failure, mapped to a process exit code by [`Error::exit_code`].
#[derive(Debug)]
pub enum Error {
    /// Invalid command-line usage.
    Usage(String),
    /// A problem with the environment texmake runs in.
    Environment(String),
    /// A problem with the configuration file.
    Config(ConfigError),
}

impl Error {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Environment(_) | Error::Config(_) => 2,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(message) | Error::Environment(message) => {
                write!(f, "error: {message}")
            }
            // Config errors carry their own file/line/column formatting.
            Error::Config(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Config(error) => Some(error),
            Error::Usage(_) | Error::Environment(_) => None,
        }
    }
}

impl From<ConfigError> for Error {
    fn from(error: ConfigError) -> Self {
        Error::Config(error)
    }
}

/// A configuration-file problem.
///
/// `Toml` keeps the parser's message, which already names the line and column.
/// `Semantic` is produced by validation once a span has been resolved to a
/// line/column pair, and prints as `{path}:{line}:{column}: error: {reason}`.
#[derive(Debug)]
pub enum ConfigError {
    /// The file is not valid TOML, or does not match the configuration schema.
    Toml {
        /// Path of the offending configuration file.
        path: PathBuf,
        /// Parser error, including its line/column block.
        source: toml::de::Error,
    },
    /// The file parses but violates a semantic rule.
    // Constructed by validation in a later task.
    #[allow(dead_code)]
    Semantic {
        /// Path of the offending configuration file.
        path: PathBuf,
        /// 1-based line number of the offending value.
        line: usize,
        /// 1-based column number of the offending value.
        column: usize,
        /// Human-readable reason.
        reason: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Toml { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            ConfigError::Semantic {
                path,
                line,
                column,
                reason,
            } => write!(f, "{}:{line}:{column}: error: {reason}", path.display()),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Toml { source, .. } => Some(source),
            ConfigError::Semantic { .. } => None,
        }
    }
}
