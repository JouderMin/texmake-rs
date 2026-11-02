// texmake - a make-like build tool for LaTeX document projects
// Copyright (C) 2026 texmake contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Error types shared by every texmake subcommand.
//!
//! Domain failures (`Usage`, `Config`, `ToolMissing`, `BuildFailed`,
//! `Member`, `Cancelled`) are defined here with `thiserror`, and every one of them knows
//! its process exit code: usage/configuration problems exit with 2, build
//! failures (missing tools, failed steps) with 1, and a cancelled interactive
//! prompt with 130. Environment and I/O failures are not modelled as a variant;
//! the code that hits them attaches an `anyhow` context instead, and
//! [`Error::exit_code`] keeps exit codes for the domain errors.

use std::path::PathBuf;

/// A texmake failure, mapped to a process exit code by [`Error::exit_code`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid command-line usage.
    #[error("error: {0}")]
    Usage(String),
    /// A problem with the configuration file.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// A build step needed an executable that is not available.
    #[error("error: missing required executable(s):{}", render_tools(.0))]
    ToolMissing(Vec<String>),
    /// A build step ran and failed.
    #[error("{0}")]
    BuildFailed(#[from] BuildFailed),
    /// A workspace member failed; the inner error keeps its exit code.
    #[error("member {member}: {source}")]
    Member {
        /// The member path as written in the workspace configuration.
        member: String,
        /// The member's own failure.
        source: Box<Error>,
    },
    /// The user cancelled an interactive prompt (Esc, Ctrl-C or end of input).
    #[error("已取消")]
    Cancelled,
}

impl Error {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Config(_) => 2,
            Error::ToolMissing(_) | Error::BuildFailed(_) => 1,
            Error::Member { source, .. } => source.exit_code(),
            Error::Cancelled => 130,
        }
    }
}

/// Wrap a usage error for the `anyhow`-based command layer.
pub fn usage(message: String) -> anyhow::Error {
    Error::Usage(message).into()
}

/// Render the missing-tool list one executable per line.
fn render_tools(tools: &[String]) -> String {
    tools.iter().map(|tool| format!("\n  {tool}")).collect()
}

/// A build step that ran and exited unsuccessfully.
///
/// The build prints it as a fixed report: the rendered command, the exit code
/// (or terminating signal), the last lines of the step's merged output, and
/// the path of the complete log.
#[derive(Debug, thiserror::Error)]
#[error(
    "error: step failed\n  command: {command}\n  exit code: {}\n  last 20 lines:{}\n  log: {}",
    Termination::new(*exit_code, *signal).report_line(),
    tail_lines(tail),
    log.display()
)]
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

/// How a step ended: an exit code, a terminating signal, or neither.
///
/// The build log and the failure report render the same fact differently, so
/// both renderings live here next to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Termination {
    exit_code: Option<i32>,
    signal: Option<i32>,
}

impl Termination {
    pub(crate) fn new(exit_code: Option<i32>, signal: Option<i32>) -> Self {
        Self { exit_code, signal }
    }

    /// The `exit code:` value of a failure report, including the signal note
    /// when one is known.
    pub(crate) fn report_line(self) -> String {
        match (self.exit_code, self.signal) {
            (_, Some(signal)) => format!("signal {signal}\n  terminated by signal {signal}"),
            (Some(code), None) => code.to_string(),
            (None, None) => "unknown".to_owned(),
        }
    }

    /// The bracketed status line that closes a step in the build log.
    pub(crate) fn log_line(self) -> String {
        match (self.exit_code, self.signal) {
            (_, Some(signal)) => format!("[signal {signal}]"),
            (Some(code), None) => format!("[{code}]"),
            (None, None) => "[unknown]".to_owned(),
        }
    }
}

/// Render the output tail one indented line per entry.
fn tail_lines(tail: &[String]) -> String {
    tail.iter().map(|line| format!("\n    {line}")).collect()
}

/// A configuration-file problem.
///
/// `Toml` keeps the parser's message, which already names the line and column.
/// `Semantic` is produced by validation once a span has been resolved to a
/// line/column pair, and prints as `{path}:{line}:{column}: error: {reason}`.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file is not valid TOML, or does not match the configuration schema.
    #[error("{}: {source}", path.display())]
    Toml {
        /// Path of the offending configuration file.
        path: PathBuf,
        /// Parser error, including its line/column block.
        source: toml::de::Error,
    },
    /// The file parses but violates a semantic rule.
    #[error("{}:{line}:{column}: error: {reason}", path.display())]
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
