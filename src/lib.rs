#![cfg_attr(test, allow(clippy::unwrap_used))]

//! texmake library.
//!
//! The binary is a thin shell around [`cli::run`]; the modules here hold the
//! configuration model, validation, step resolution and the build pipeline, so
//! integration tests and benches can drive them directly.

pub mod build;
pub mod clean;
pub mod cli;
pub mod config;
pub mod engine;
pub mod error;
pub mod init;
pub mod paths;
pub mod recipe;
pub mod up_to_date;
pub mod validate;
pub mod vars;

use anyhow::Context;

use crate::cli::Cli;
use crate::config::LoadedConfig;

/// Discover and load the project configuration for `cli`.
pub fn load_project(cli: &Cli) -> anyhow::Result<LoadedConfig> {
    let cwd = std::env::current_dir().context("cannot read working directory")?;
    let path = config::discover(cli.file.as_deref(), &cwd)?;
    config::load(&path)
}
