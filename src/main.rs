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

#![cfg_attr(test, allow(clippy::unwrap_used))]

//! texmake entry point.
//!
//! The crate ships a single binary: `main` parses the command line, delegates
//! to [`cli::run`] and turns the result into the process exit code. The modules
//! below hold the configuration model, validation, step resolution and the
//! build pipeline.

mod build;
mod clean;
mod cli;
mod config;
mod engine;
mod error;
mod init;
mod paths;
mod prompt;
mod recipe;
mod up_to_date;
mod validate;
mod vars;

use anyhow::Context;

use crate::cli::Cli;
use crate::config::LoadedConfig;

/// Discover and load the project configuration for `cli`.
fn load_project(cli: &Cli) -> anyhow::Result<LoadedConfig> {
    let cwd = std::env::current_dir().context("cannot read working directory")?;
    let path = config::discover(cli.file.as_deref(), &cwd)?;
    config::load(&path)
}

fn main() {
    std::process::exit(cli::run());
}
