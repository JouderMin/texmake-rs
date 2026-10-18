//! texmake entry point.
//!
//! `main` delegates argument parsing, command dispatch and error printing to
//! [`cli::run`]; [`load_project`] is the shared configuration step that every
//! project command performs before doing its work.

mod build;
mod clean;
mod cli;
mod config;
mod error;
mod init;
mod paths;
mod recipe;
mod up_to_date;
mod validate;
mod vars;

use anyhow::Context;

use crate::cli::Cli;
use crate::config::LoadedConfig;

fn main() {
    std::process::exit(cli::run());
}

/// Discover and load the project configuration for `cli`.
pub fn load_project(cli: &Cli) -> anyhow::Result<LoadedConfig> {
    let cwd = std::env::current_dir().context("cannot read working directory")?;
    let path = config::discover(cli.file.as_deref(), &cwd)?;
    config::load(&path)
}
