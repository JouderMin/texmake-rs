//! Removal of every recipe's output directory.
//!
//! The targets come from the validated [`Project`], so `build_plan` has
//! already refused output directories that are the project root, outside the
//! root, a regular file, or that contain a main document. `clean` only has to
//! deduplicate the targets and remove them without following symbolic links
//! inside them.

use std::io;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::paths;
use crate::validate::Project;

/// Remove all recipe output directories of `project`.
///
/// Nested output directories are removed through their parent only. Missing
/// directories are skipped: there is nothing to clean. With `dry_run` the
/// targets are printed instead of removed; `quiet` suppresses the `removed`
/// lines but never dry-run output.
pub fn run_clean(project: &Project, dry_run: bool, quiet: bool) -> Result<(), Error> {
    let targets: Vec<PathBuf> = project
        .recipes
        .values()
        .map(|recipe| recipe.output_path.clone())
        .collect();

    for target in paths::dedupe_nested(targets) {
        let shown = display_path(&project.root, &target);
        let shown = shown.display();
        match std::fs::symlink_metadata(&target) {
            // Removing a symbolic link would leave its target in place while
            // reporting a successful clean, so refuse instead.
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Usage(format!(
                    "refusing to remove `{shown}`: it is a symbolic link"
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::Usage(format!(
                    "refusing to remove `{shown}`: it is not a directory"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(Error::Environment(format!(
                    "cannot inspect `{shown}`: {error}"
                )));
            }
        }

        if dry_run {
            println!("would remove {shown}");
            continue;
        }

        // `remove_dir_all` does not follow symbolic links inside the directory:
        // it unlinks them, so a link pointing outside the project stays intact.
        std::fs::remove_dir_all(&target)
            .map_err(|error| Error::Environment(format!("cannot remove `{shown}`: {error}")))?;
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
