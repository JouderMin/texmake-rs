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
    let Ok(pdf_mtime) = fs::metadata(&pdf).and_then(|metadata| metadata.modified()) else {
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
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if entry.file_name() == ".git" || outputs.contains(&path.as_path()) {
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
