//! Build orchestration.
//!
//! This task completes the build up to and including the up-to-date check. A
//! recipe whose PDF is newer than its sources is skipped; everything else runs
//! its steps through the minimal executor below so the skip check has a real
//! build to compare against. Logging, tool checks and failure reports arrive in
//! Task 7.

use std::process::Command;

use crate::error::Error;
use crate::recipe;
use crate::up_to_date;
use crate::validate::{Project, Recipe};

/// Options controlling one build, taken from the global CLI flags.
pub struct BuildOptions {
    /// Ignore the up-to-date check.
    pub force: bool,
    /// Print commands without executing them.
    pub dry_run: bool,
    /// Print full commands and tool output.
    #[allow(dead_code)] // read by the build in a later task
    pub verbose: bool,
    /// Print errors only.
    pub quiet: bool,
}

/// Build `recipe`, or print its commands when `opts.dry_run` is set.
///
/// `--dry-run` prints every command even with `--quiet` and performs no
/// up-to-date check, no output-directory creation and no tool check.
///
/// # Errors
///
/// Returns an environment error when the output directory or a step cannot be
/// started or a step exits unsuccessfully.
pub fn run_build(project: &Project, recipe: &Recipe, opts: &BuildOptions) -> Result<(), Error> {
    if opts.dry_run {
        for step in recipe::resolve_steps(project, recipe) {
            println!("{}", step.display);
        }
        return Ok(());
    }
    if !opts.force && up_to_date::is_up_to_date(project, recipe) {
        if !opts.quiet {
            println!("Everything is up to date.");
        }
        return Ok(());
    }
    execute(project, recipe)
}

/// Run every step of `recipe` in order, creating the output directory first.
///
/// Task 7 extends this with log truncation, tool checks and the failure report;
/// for now a step's exit status only decides between success and an error.
fn execute(project: &Project, recipe: &Recipe) -> Result<(), Error> {
    std::fs::create_dir_all(&recipe.output_path).map_err(|error| {
        Error::Environment(format!(
            "cannot create `{}`: {error}",
            recipe.output_path.display()
        ))
    })?;
    for step in recipe::resolve_steps(project, recipe) {
        // Every resolved step currently carries a command; the option leaves
        // room for steps that only exist to be printed.
        let Some(exec) = step.exec else {
            continue;
        };
        let status = Command::new(&exec.program)
            .args(&exec.args)
            .current_dir(&exec.cwd)
            .status()
            .map_err(|error| {
                Error::Environment(format!("cannot start `{}`: {error}", exec.program))
            })?;
        if !status.success() {
            return Err(Error::Environment(format!(
                "step `{}` failed with {status}",
                step.display
            )));
        }
    }
    Ok(())
}
