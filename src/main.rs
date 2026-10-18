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

use crate::cli::Cli;
use crate::config::LoadedConfig;
use crate::error::Error;

fn main() {
    std::process::exit(cli::run());
}

/// Discover and load the project configuration for `cli`.
pub fn load_project(cli: &Cli) -> Result<LoadedConfig, Error> {
    let cwd = std::env::current_dir()
        .map_err(|error| Error::Environment(format!("cannot read working directory: {error}")))?;
    let path = config::discover(cli.file.as_deref(), &cwd)?;
    config::load(&path)
}
