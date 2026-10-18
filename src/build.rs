//! Build orchestration.
//!
//! This task implements the `--dry-run` branch only: after validation it prints
//! the resolved command lines without creating the output directory, checking
//! tools or running anything. The remaining build phases arrive in later tasks.

use crate::error::Error;
use crate::recipe;
use crate::validate::{Project, Recipe};

/// Options controlling one build, taken from the global CLI flags.
pub struct BuildOptions {
    /// Ignore the up-to-date check.
    #[allow(dead_code)] // read by the build in a later task
    pub force: bool,
    /// Print commands without executing them.
    pub dry_run: bool,
    /// Print full commands and tool output.
    #[allow(dead_code)] // read by the build in a later task
    pub verbose: bool,
    /// Print errors only.
    #[allow(dead_code)] // read by the build in a later task
    pub quiet: bool,
}

/// Build `recipe`, or print its commands when `opts.dry_run` is set.
///
/// `--dry-run` prints every command even with `--quiet` and performs no
/// up-to-date check, no output-directory creation and no tool check.
///
/// # Errors
///
/// The non-dry-run build is not implemented yet.
pub fn run_build(project: &Project, recipe: &Recipe, opts: &BuildOptions) -> Result<(), Error> {
    if opts.dry_run {
        for step in recipe::resolve_steps(project, recipe) {
            println!("{}", step.display);
        }
        return Ok(());
    }
    Err(Error::Usage("not implemented".to_owned()))
}
