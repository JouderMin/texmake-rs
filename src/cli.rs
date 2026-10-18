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

//! Command-line interface definition.
//!
//! The CLI shape is the contract later tasks build on: global options are
//! handled uniformly, and the subcommand variants carry the raw argument
//! values. `init` scaffolds a new project; `check`, the build commands and
//! `clean` operate on an existing configuration.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::build::{self, BuildOptions};
use crate::clean;
use crate::config::LoadedConfig;
use crate::engine::Engine;
use crate::error::Error;
use crate::init::{self, Language};
use crate::recipe;
use crate::validate::{self, Project};

/// texmake: a make-like build tool for LaTeX document projects.
#[derive(Debug, Parser)]
#[command(
    name = "texmake",
    version,
    about = "A make-like build tool for LaTeX document projects",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Use the given configuration file instead of discovering texmake.toml.
    #[arg(short = 'f', long, global = true, value_name = "path")]
    pub file: Option<PathBuf>,

    /// Ignore the up-to-date check.
    #[arg(short = 'B', long, global = true)]
    pub force: bool,

    /// Print every command without executing it.
    #[arg(short = 'n', long, global = true)]
    pub dry_run: bool,

    /// Print full commands and tool output.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Print errors only.
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build the default or the named recipe.
    Build {
        /// Recipe name; defaults to the project default recipe.
        name: Option<String>,
    },

    /// Initialize a new document project.
    Init {
        /// Target directory; defaults to the current directory.
        dir: Option<PathBuf>,

        /// Project name; defaults to the target directory name.
        #[arg(long)]
        name: Option<String>,

        /// LaTeX engine used by the generated recipe.
        #[arg(long, value_enum, default_value = "xelatex")]
        engine: Engine,

        /// Document language of the generated main.tex.
        #[arg(long, value_enum, default_value = "zh")]
        language: Language,
    },

    /// Remove every recipe's output directory.
    Clean,

    /// Validate the configuration file.
    Check,
}

/// Parse the command line and run texmake; returns the process exit code.
pub fn run() -> i32 {
    let cli = Cli::parse();
    match execute(cli) {
        Ok(()) => 0,
        // Domain errors keep their exact text and exit code. Anything else is
        // an `anyhow` context chain (I/O and environment failures); it exits
        // with 2 and is printed with the same `error: ` prefix.
        Err(error) => match error.downcast_ref::<Error>() {
            Some(domain) => {
                eprintln!("{domain}");
                domain.exit_code()
            }
            None => {
                eprintln!("error: {error:#}");
                2
            }
        },
    }
}

fn execute(cli: Cli) -> anyhow::Result<()> {
    // `init` creates a new project and must not require an existing
    // configuration; every other command loads the project first.
    if let Some(Command::Init {
        dir,
        name,
        engine,
        language,
    }) = &cli.command
    {
        let options = init::InitOptions {
            dir: dir.clone().unwrap_or_else(|| PathBuf::from(".")),
            name: name.clone(),
            engine: *engine,
            language: *language,
            dry_run: cli.dry_run,
            quiet: cli.quiet,
        };
        return init::run_init(&options);
    }
    let loaded = crate::load_project(&cli)?;
    match &cli.command {
        Some(Command::Check) => {
            let project = validate::build_plan(&loaded).map_err(Error::from)?;
            print_check(&project);
            Ok(())
        }
        Some(Command::Build { name }) => build_command(&cli, &loaded, name.as_deref()),
        Some(Command::Clean) => clean_command(&cli, &loaded),
        // Handled before the configuration is loaded.
        Some(Command::Init { .. }) => unreachable!("init does not load the project"),
        None => build_command(&cli, &loaded, None),
    }
}

/// Validate the project and remove every recipe's output directory.
fn clean_command(cli: &Cli, loaded: &LoadedConfig) -> anyhow::Result<()> {
    let project = validate::build_plan(loaded).map_err(Error::from)?;
    clean::run_clean(&project, cli.dry_run, cli.quiet)
}

/// Validate the project, select the recipe and run the build.
fn build_command(cli: &Cli, loaded: &LoadedConfig, name: Option<&str>) -> anyhow::Result<()> {
    let project = validate::build_plan(loaded).map_err(Error::from)?;
    let recipe = recipe::select_recipe(&project, name)?;
    let options = BuildOptions {
        force: cli.force,
        dry_run: cli.dry_run,
        verbose: cli.verbose,
        quiet: cli.quiet,
    };
    build::run_build(&project, recipe, &options)
}

/// Report what `texmake check` validated.
fn print_check(project: &Project) {
    println!("project: {}", project.name);
    println!("recipes:");
    for name in project.recipes.keys() {
        if project.default.as_deref() == Some(name.as_str()) {
            println!("  {name} (default)");
        } else {
            println!("  {name}");
        }
    }
}
