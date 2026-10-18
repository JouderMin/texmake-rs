//! Command-line interface definition.
//!
//! The CLI shape is the contract later tasks build on: global options are
//! handled uniformly, and the subcommand variants carry the raw argument
//! values. This task only parses arguments; every subcommand still reports
//! "not implemented".

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::error::Error;

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

/// LaTeX engines supported by `texmake init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Engine {
    Xelatex,
    Pdflatex,
    Lualatex,
}

/// Document languages supported by `texmake init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Language {
    Zh,
    En,
}

/// Parse the command line and run texmake; returns the process exit code.
pub fn run() -> i32 {
    let cli = Cli::parse();
    match execute(cli) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn execute(_cli: Cli) -> Result<(), Error> {
    Err(Error::Usage("not implemented".to_owned()))
}
