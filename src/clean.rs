//! Removal of every recipe's output directory.
//!
//! The targets come from the validated [`Project`], so `build_plan` has
//! already refused output directories that are the project root, outside the
//! root, a regular file, or that contain a main document. `clean` additionally
//! refuses an `output` that is itself a symbolic link: its validated
//! `output_path` is canonicalized, so removing that path would delete the link
//! target and leave the link dangling. The remaining targets are deduplicated
//! and removed without following symbolic links inside them.

use std::io;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::error::usage;
use crate::paths;
use crate::validate::Project;

/// Remove all recipe output directories of `project`.
///
/// Nested output directories are removed through their parent only. Missing
/// directories are skipped: there is nothing to clean. With `dry_run` the
/// targets are printed instead of removed; `quiet` suppresses the `removed`
/// lines but never dry-run output.
pub fn run_clean(project: &Project, dry_run: bool, quiet: bool) -> anyhow::Result<()> {
    // Refuse a symlinked `output` before anything is printed or removed.
    // `output_path` is canonicalized, so removing it would delete the link
    // target and leave the link dangling; deleting the link instead could
    // discard a directory the link shares with other projects.
    for recipe in project.recipes.values() {
        // Normalize first so a trailing slash cannot make the final component
        // be dereferenced by `symlink_metadata`.
        let raw = paths::normalize(&project.root.join(&recipe.output));
        let shown = display_path(&project.root, &raw);
        if let Ok(metadata) = std::fs::symlink_metadata(&raw)
            && metadata.file_type().is_symlink()
        {
            return Err(usage(format!(
                "refusing to remove `{}`: it is a symbolic link; point `output` in texmake.toml at the real directory",
                shown.display()
            )));
        }
    }

    let targets: Vec<PathBuf> = project
        .recipes
        .values()
        .map(|recipe| recipe.output_path.clone())
        .collect();

    for target in paths::dedupe_nested(targets) {
        let shown = display_path(&project.root, &target);
        let shown = shown.display();
        match std::fs::symlink_metadata(&target) {
            // `output_path` is canonicalized and a symlinked `output` was
            // refused above, so this target cannot be a symbolic link.
            Ok(metadata) if !metadata.is_dir() => {
                return Err(usage(format!(
                    "refusing to remove `{shown}`: it is not a directory"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("cannot inspect `{shown}`"));
            }
        }

        if dry_run {
            println!("would remove {shown}");
            continue;
        }

        // `remove_dir_all` does not follow symbolic links inside the directory:
        // it unlinks them, so a link pointing outside the project stays intact.
        std::fs::remove_dir_all(&target).with_context(|| format!("cannot remove `{shown}`"))?;
        if !quiet {
            println!("removed {shown}");
        }
    }

    Ok(())
}

/// The path to show in messages, relative to the project root when possible.
fn display_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}
