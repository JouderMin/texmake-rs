//! The "everything is up to date" check.
//!
//! A recipe with a `latex` step is skipped when its main PDF is strictly newer
//! than every regular file under the project root, the loaded configuration
//! file included. Output directories of all recipes and `.git` are ignored, and
//! symbolic links are never followed.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::validate::{Project, Recipe, Step};

/// Whether `recipe`'s main PDF is newer than every source file.
///
/// Recipes without a `latex` step are never considered up to date.
pub fn is_up_to_date(project: &Project, recipe: &Recipe) -> bool {
    if !recipe
        .steps
        .iter()
        .any(|step| matches!(step, Step::Latex { .. }))
    {
        return false;
    }
    let pdf = recipe.output_path.join(format!("{}.pdf", recipe.main_stem));
    // A directory named like the PDF must not count as a fresh build result.
    let Ok(metadata) = fs::metadata(&pdf) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    let Ok(pdf_mtime) = metadata.modified() else {
        return false;
    };
    match newest_source_mtime(project) {
        Some(source_mtime) => pdf_mtime > source_mtime,
        None => true,
    }
}

/// The newest modification time of the project's sources, if any exist.
///
/// Walks `project.root` recursively without following symbolic links, skipping
/// `.git` and the output directories of every recipe, then compares the loaded
/// configuration file as well.
pub fn newest_source_mtime(project: &Project) -> Option<SystemTime> {
    let outputs: Vec<&Path> = project
        .recipes
        .values()
        .map(|recipe| recipe.output_path.as_path())
        .collect();
    let mut newest = None;
    visit(&project.root, &outputs, &mut newest);
    if let Ok(modified) =
        fs::metadata(&project.config_path).and_then(|metadata| metadata.modified())
    {
        newest = Some(newest.map_or(modified, |current: SystemTime| current.max(modified)));
    }
    newest
}

fn visit(directory: &Path, outputs: &[&Path], newest: &mut Option<SystemTime>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        // `DirEntry::metadata` does not follow symbolic links, so it matches
        // `symlink_metadata` while avoiding a fresh path allocation per entry.
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            // Only directories need their path, for the ignore check and the
            // recursive call; regular files stay allocation free.
            let path = entry.path();
            if outputs.contains(&path.as_path()) {
                continue;
            }
            visit(&path, outputs, newest);
        } else if file_type.is_file()
            && let Ok(modified) = metadata.modified()
        {
            *newest = Some(newest.map_or(modified, |current: SystemTime| current.max(modified)));
        }
    }
}
