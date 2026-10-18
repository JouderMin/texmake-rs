//! Error types shared by every texmake subcommand.
//!
//! Every error knows its process exit code: usage/environment/config problems
//! exit with 2, build failures (missing tools, failed steps) with 1.
//! `exit_code` stays the single mapping point.

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
    /// A build step needed an executable that is not available.
    ToolMissing(Vec<String>),
    /// A build step ran and failed.
    BuildFailed(BuildFailed),
}

impl Error {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Environment(_) | Error::Config(_) => 2,
            Error::ToolMissing(_) | Error::BuildFailed(_) => 1,
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
            Error::ToolMissing(tools) => {
                write!(f, "error: missing required executable(s):")?;
                for tool in tools {
                    write!(f, "\n  {tool}")?;
                }
                Ok(())
            }
            Error::BuildFailed(failure) => write!(f, "{failure}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Config(error) => Some(error),
            Error::Usage(_)
            | Error::Environment(_)
            | Error::ToolMissing(_)
            | Error::BuildFailed(_) => None,
        }
    }
}

/// A build step that ran and exited unsuccessfully.
///
/// The build prints it as a fixed report: the rendered command, the exit code
/// (or terminating signal), the last lines of the step's merged output, and
/// the path of the complete log.
#[derive(Debug)]
pub struct BuildFailed {
    /// Rendered command line of the failing step.
    pub command: String,
    /// Exit code when the process exited normally.
    pub exit_code: Option<i32>,
    /// Number of the signal that terminated the process, on Unix.
    pub signal: Option<i32>,
    /// Last lines of the step's merged standard output and standard error.
    pub tail: Vec<String>,
    /// Path of the complete build log.
    pub log: PathBuf,
}

impl fmt::Display for BuildFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error: step failed")?;
        writeln!(f, "  command: {}", self.command)?;
        match (self.exit_code, self.signal) {
            (_, Some(signal)) => {
                writeln!(f, "  exit code: signal {signal}")?;
                writeln!(f, "  terminated by signal {signal}")?;
            }
            (Some(code), None) => writeln!(f, "  exit code: {code}")?,
            (None, None) => writeln!(f, "  exit code: unknown")?,
        }
        writeln!(f, "  last 20 lines:")?;
        for line in &self.tail {
            writeln!(f, "    {line}")?;
        }
        write!(f, "  log: {}", self.log.display())
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
