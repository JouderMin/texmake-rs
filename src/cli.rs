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

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::build::{self, BuildOptions};
use crate::clean;
use crate::config::{self, LoadedConfig, LoadedWorkspace};
use crate::error::Error;
use crate::init;
use crate::recipe;
use crate::validate::{self, Project};
use crate::workspace::{self, WorkspaceMember};

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

    /// Only process the named workspace members; may be repeated.
    #[arg(short = 'p', long = "project", global = true, value_name = "name")]
    pub project: Vec<String>,

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
    Init,

    /// Remove every recipe's output directory.
    Clean,

    /// Validate the configuration file.
    Check,
}

/// Parse the command line and run texmake; returns the process exit code.
pub fn run() -> i32 {
    // `parse` prints the help, the version or the usage error itself and then
    // exits the process, so a parse failure never reaches `execute`.
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

/// Discover and load the project configuration for `cli`.
fn load_project(cli: &Cli) -> anyhow::Result<LoadedConfig> {
    let cwd = std::env::current_dir().context("cannot read working directory")?;
    let path = config::discover(cli.file.as_deref(), &cwd)?;
    config::load(&path)
}

fn execute(cli: Cli) -> anyhow::Result<()> {
    // `init` creates a new project and must not require an existing
    // configuration; every other command loads the project first.
    if matches!(cli.command, Some(Command::Init)) {
        reject_project_flag(&cli)?;
        let options = init::InitOptions {
            dry_run: cli.dry_run,
            quiet: cli.quiet,
        };
        return init::run_init(&options);
    }
    let loaded = load_project(&cli)?;
    if matches!(&loaded, LoadedConfig::Project(_)) {
        reject_project_flag(&cli)?;
    }
    match &cli.command {
        Some(Command::Check) => check_command(&cli, &loaded),
        Some(Command::Build { name }) => build_command(&cli, &loaded, name.as_deref()),
        Some(Command::Clean) => clean_command(&cli, &loaded),
        // Handled before the configuration is loaded.
        Some(Command::Init) => unreachable!("init does not load the project"),
        None => build_command(&cli, &loaded, None),
    }
}

/// Validate the configuration and report what was checked.
///
/// A workspace prints one `==> {member}` heading and one report per member.
fn check_command(cli: &Cli, loaded: &LoadedConfig) -> anyhow::Result<()> {
    match loaded {
        LoadedConfig::Project(loaded) => {
            let project = validate::build_plan(loaded).map_err(Error::from)?;
            print_check(&project);
            Ok(())
        }
        LoadedConfig::Workspace(loaded) => for_each_member(cli, loaded, |member| {
            print_check(&member.project);
            Ok(())
        }),
    }
}

/// Validate the project(s) and remove every recipe's output directory.
fn clean_command(cli: &Cli, loaded: &LoadedConfig) -> anyhow::Result<()> {
    match loaded {
        LoadedConfig::Project(loaded) => {
            let project = validate::build_plan(loaded).map_err(Error::from)?;
            clean::run_clean(&project, cli.dry_run, cli.quiet)
        }
        LoadedConfig::Workspace(loaded) => for_each_member(cli, loaded, |member| {
            clean::run_clean(&member.project, cli.dry_run, cli.quiet)
                .map_err(|error| member_context(&member.display, error))
        }),
    }
}

/// Validate the project(s), select the recipe and run the build(s).
fn build_command(cli: &Cli, loaded: &LoadedConfig, name: Option<&str>) -> anyhow::Result<()> {
    let options = BuildOptions {
        force: cli.force,
        dry_run: cli.dry_run,
        verbose: cli.verbose,
        quiet: cli.quiet,
    };
    match loaded {
        LoadedConfig::Project(loaded) => {
            let project = validate::build_plan(loaded).map_err(Error::from)?;
            let recipe = recipe::select_recipe(&project, name)?;
            build::run_build(&project, recipe, &options)
        }
        LoadedConfig::Workspace(loaded) => for_each_member(cli, loaded, |member| {
            let recipe = recipe::select_recipe(&member.project, name)
                .map_err(|error| member_context(&member.display, error.into()))?;
            build::run_build(&member.project, recipe, &options)
                .map_err(|error| member_context(&member.display, error))
        }),
    }
}

/// Run `action` once per selected workspace member, under its heading.
///
/// The workspace is validated and its member selection resolved first; an
/// empty selection is a usage error.
fn for_each_member(
    cli: &Cli,
    loaded: &LoadedWorkspace,
    mut action: impl FnMut(&WorkspaceMember) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let workspace = workspace::build_workspace(loaded).map_err(Error::from)?;
    require_members(&workspace)?;
    for member in workspace::select_members(&workspace, &cli.project)? {
        heading(cli, &member.display);
        action(member)?;
    }
    Ok(())
}

/// Reject `-p` outside a workspace.
fn reject_project_flag(cli: &Cli) -> anyhow::Result<()> {
    if cli.project.is_empty() {
        return Ok(());
    }
    Err(crate::error::usage(
        "`-p` is only valid in a workspace".to_owned(),
    ))
}

/// Print the `==> {member}` heading that separates workspace members.
fn heading(cli: &Cli, member: &str) {
    if !cli.quiet {
        println!("==> {member}");
    }
}

/// Reject a workspace that lists no members.
fn require_members(workspace: &workspace::Workspace) -> anyhow::Result<()> {
    if workspace.members.is_empty() {
        return Err(crate::error::usage(format!(
            "workspace `{}` has no members",
            workspace.root.display()
        )));
    }
    Ok(())
}

/// Prefix a member failure with `member {name}: ` context.
///
/// A domain error is wrapped so [`Error::exit_code`] keeps its exit code; an
/// I/O or environment error is flattened into a usage-style message.
fn member_context(member: &str, error: anyhow::Error) -> anyhow::Error {
    match error.downcast::<Error>() {
        Ok(domain) => Error::Member {
            member: member.to_owned(),
            source: Box::new(domain),
        }
        .into(),
        Err(other) => anyhow::anyhow!("member {member}: {other:#}"),
    }
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
