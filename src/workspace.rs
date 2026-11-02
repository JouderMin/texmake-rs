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

//! Workspace roots: member resolution, validation and `-p` selection.
//!
//! A workspace root is a `texmake.toml` with a `[workspace]` table. Its
//! members are resolved relative to the root, validated as independent
//! projects, and selected by `[project] name` with `-p`.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::config::{FILE_NAME, LoadedConfig, LoadedProject, LoadedWorkspace};
use crate::error::{ConfigError, Error};
use crate::paths;
use crate::validate::{Project, build_plan};

/// A validated workspace root.
#[derive(Debug)]
pub struct Workspace {
    /// Absolute, canonical workspace root (the directory of the root config).
    pub root: PathBuf,
    /// Member projects, in the order they are listed.
    pub members: Vec<WorkspaceMember>,
}

/// One validated workspace member.
#[derive(Debug)]
pub struct WorkspaceMember {
    /// The member path exactly as written in `members`; used as the
    /// `==> {member}` heading and in error context.
    pub display: String,
    /// The member's validated project.
    pub project: Project,
}

/// Validate a workspace root and every project it lists.
///
/// Members are resolved relative to the workspace root, must stay inside it,
/// must be unique, must be directories that contain a `texmake.toml`, and that
/// file must describe a project: nested workspaces are rejected. Member
/// failures carry `member {name}: ` context in front of the member's own
/// positioned error.
pub fn build_workspace(loaded: &LoadedWorkspace) -> Result<Workspace, ConfigError> {
    let span = loaded.workspace.members.span();
    let root = loaded.file.root.canonicalize().map_err(|error| {
        loaded.file.error_at(
            span.clone(),
            format!("cannot resolve the workspace root: {error}"),
        )
    })?;
    let mut members = Vec::new();
    let mut seen = BTreeSet::new();
    let mut names: BTreeMap<String, PathBuf> = BTreeMap::new();

    for indexed in loaded.workspace.members.get_ref() {
        let display = indexed.get_ref().clone();
        let context = MemberContext {
            loaded,
            root: &root,
            display: &display,
            at: indexed.span(),
        };
        let dir = context.member_dir()?;
        if !seen.insert(dir.clone()) {
            return Err(context.fail(format!("duplicate workspace member `{display}`")));
        }
        let project = context.load_project(&dir)?;
        if let Some(previous) = names.insert(project.name.clone(), project.config_path.clone()) {
            return Err(context.fail(format!(
                "workspace has duplicate project name `{}`: {}, {}",
                project.name,
                previous.display(),
                project.config_path.display()
            )));
        }
        members.push(WorkspaceMember { display, project });
    }

    Ok(Workspace { root, members })
}

/// One `members` entry while it is validated, with the root it must stay in.
struct MemberContext<'a> {
    loaded: &'a LoadedWorkspace,
    root: &'a Path,
    display: &'a str,
    at: Range<usize>,
}

impl MemberContext<'_> {
    /// A positioned error about this member.
    fn fail(&self, reason: String) -> ConfigError {
        self.loaded.file.error_at(self.at.clone(), reason)
    }

    /// Check the raw path and return it normalized, before symlink resolution.
    fn member_dir(&self) -> Result<PathBuf, ConfigError> {
        if self.display.is_empty() {
            return Err(self.fail("a workspace member path cannot be empty".to_owned()));
        }
        let relative = Path::new(self.display);
        if relative.is_absolute() {
            return Err(self.fail(format!(
                "workspace member `{}` must be a relative path",
                self.display
            )));
        }
        let dir = paths::normalize(&self.root.join(relative));
        if dir.as_path() == self.root || !dir.starts_with(self.root) {
            return Err(self.fail(format!(
                "workspace member `{}` must stay inside the workspace root",
                self.display
            )));
        }
        Ok(dir)
    }

    /// Resolve the member directory and load the project it contains.
    fn load_project(&self, dir: &Path) -> Result<Project, ConfigError> {
        // Resolve symlinks before trusting the member: a link can point out of
        // the workspace, and a dangling or looping link cannot be validated.
        let resolved = dir
            .canonicalize()
            .map_err(|error| self.cannot_resolve(dir, error))?;
        if resolved.as_path() == self.root {
            return Err(self.fail(format!(
                "workspace member `{}`: `{}` is the workspace root",
                self.display,
                resolved.display()
            )));
        }
        if !resolved.starts_with(self.root) {
            return Err(self.fail(format!(
                "workspace member `{}`: `{}` resolves outside the workspace root",
                self.display,
                resolved.display()
            )));
        }
        if !resolved.is_dir() {
            return Err(self.fail(format!(
                "workspace member `{}` is not a directory",
                self.display
            )));
        }
        let config_path = resolved.join(FILE_NAME);
        if !config_path.is_file() {
            return Err(self.fail(format!(
                "workspace member `{}` has no {FILE_NAME}",
                self.display
            )));
        }
        let config_path = config_path
            .canonicalize()
            .map_err(|error| self.cannot_resolve(&config_path, error))?;
        if config_path.as_path() == self.root || !config_path.starts_with(self.root) {
            return Err(self.fail(format!(
                "workspace member `{}`: `{}` resolves outside the workspace root",
                self.display,
                config_path.display()
            )));
        }
        let member = self.read_project(&config_path)?;
        build_plan(&member).map_err(|reason| self.member_fail(&reason))
    }

    /// Load the member configuration, rejecting a nested workspace.
    fn read_project(&self, config_path: &Path) -> Result<LoadedProject, ConfigError> {
        let config = crate::config::load(config_path)
            .map_err(|error| member_error(self.loaded, self.at.clone(), self.display, &error))?;
        match config {
            LoadedConfig::Project(project) => Ok(project),
            LoadedConfig::Workspace(nested) => {
                let reason = nested.file.error_at(
                    nested.workspace.members.span(),
                    "nested workspaces are not supported",
                );
                Err(self.member_fail(&reason))
            }
        }
    }

    /// A positioned `member {name}: {reason}` error.
    fn member_fail(&self, reason: &impl std::fmt::Display) -> ConfigError {
        self.fail(format!("member {}: {reason}", self.display))
    }

    /// A `cannot resolve` error for `path`.
    fn cannot_resolve(&self, path: &Path, error: std::io::Error) -> ConfigError {
        self.fail(format!(
            "workspace member `{}`: cannot resolve `{}`: {error}",
            self.display,
            path.display()
        ))
    }
}

/// Keep only the members selected by `-p`.
///
/// An empty selection keeps every member. Names match `[project] name`
/// exactly and case-sensitively; the result keeps the `members` order, so a
/// repeated name selects its member only once. An unknown name is a usage
/// error that lists the available project names.
pub fn select_members<'a>(
    workspace: &'a Workspace,
    names: &[String],
) -> Result<Vec<&'a WorkspaceMember>, Error> {
    if names.is_empty() {
        return Ok(workspace.members.iter().collect());
    }
    for name in names {
        if !workspace
            .members
            .iter()
            .any(|member| member.project.name == *name)
        {
            let available = workspace
                .members
                .iter()
                .map(|member| member.project.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Usage(format!(
                "unknown project `{name}`: available: {available}"
            )));
        }
    }
    Ok(workspace
        .members
        .iter()
        .filter(|member| names.contains(&member.project.name))
        .collect())
}

/// Turn a member configuration failure into a positioned workspace error.
fn member_error(
    loaded: &LoadedWorkspace,
    span: Range<usize>,
    display: &str,
    error: &anyhow::Error,
) -> ConfigError {
    match error.downcast_ref::<crate::error::Error>() {
        Some(crate::error::Error::Config(config)) => loaded
            .file
            .error_at(span, format!("member {display}: {config}")),
        _ => loaded
            .file
            .error_at(span, format!("member {display}: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Characterization: workspace member validation and `-p` selection
    // -----------------------------------------------------------------------

    fn load_workspace(dir: &Path, toml: &str) -> LoadedWorkspace {
        let path = dir.join("texmake.toml");
        std::fs::write(&path, toml).unwrap();
        match crate::config::load(&path).unwrap() {
            LoadedConfig::Workspace(workspace) => workspace,
            LoadedConfig::Project(_) => panic!("expected a workspace configuration"),
        }
    }

    fn write_project_file(dir: &Path, name: &str) {
        std::fs::write(dir.join("main.tex"), "").unwrap();
        std::fs::write(
            dir.join("texmake.toml"),
            format!(
                "[project]\nname = \"{name}\"\nmain = \"main.tex\"\n\
                 [recipes.release]\nsteps = [{{ latex = \"xelatex\" }}]\n"
            ),
        )
        .unwrap();
    }

    fn two_member_workspace(dir: &Path) -> Workspace {
        for name in ["paper", "slides"] {
            std::fs::create_dir(dir.join(name)).unwrap();
            write_project_file(&dir.join(name), name);
        }
        let loaded = load_workspace(dir, "[workspace]\nmembers = [\"paper\", \"slides\"]\n");
        build_workspace(&loaded).unwrap()
    }

    #[test]
    fn workspace_members_are_checked_in_listed_order() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load_workspace(tmp.path(), "[workspace]\nmembers = [\"\", \"/abs\"]\n");
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(error.contains("cannot be empty"), "{error}");

        let loaded = load_workspace(tmp.path(), "[workspace]\nmembers = [\"/abs\", \"\"]\n");
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(error.contains("must be a relative path"), "{error}");
    }

    #[test]
    fn a_workspace_member_must_stay_inside_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load_workspace(tmp.path(), "[workspace]\nmembers = [\"..\"]\n");
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(
            error.contains("must stay inside the workspace root"),
            "{error}"
        );
    }

    #[test]
    fn a_duplicate_workspace_member_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("paper")).unwrap();
        write_project_file(&tmp.path().join("paper"), "paper");
        let loaded = load_workspace(
            tmp.path(),
            "[workspace]\nmembers = [\"paper\", \"./paper\"]\n",
        );
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(
            error.contains("duplicate workspace member `./paper`"),
            "{error}"
        );
    }

    #[test]
    fn a_workspace_member_without_a_configuration_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("paper")).unwrap();
        let loaded = load_workspace(tmp.path(), "[workspace]\nmembers = [\"paper\"]\n");
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(
            error.contains("workspace member `paper` has no texmake.toml"),
            "{error}"
        );
    }

    #[test]
    fn a_broken_member_configuration_is_prefixed_with_the_member() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("paper")).unwrap();
        std::fs::write(
            tmp.path().join("paper/texmake.toml"),
            "not toml at all = = =",
        )
        .unwrap();
        let loaded = load_workspace(tmp.path(), "[workspace]\nmembers = [\"paper\"]\n");
        let error = build_workspace(&loaded).unwrap_err().to_string();
        assert!(error.contains("member paper: "), "{error}");
    }

    #[test]
    fn select_members_keeps_every_member_without_a_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = two_member_workspace(tmp.path());
        let selected = select_members(&workspace, &[]).unwrap();
        let names: Vec<_> = selected
            .iter()
            .map(|member| member.display.clone())
            .collect();
        assert_eq!(names, ["paper", "slides"]);
    }

    #[test]
    fn select_members_keeps_the_member_order_of_the_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = two_member_workspace(tmp.path());
        let selected =
            select_members(&workspace, &["slides".to_owned(), "paper".to_owned()]).unwrap();
        let names: Vec<_> = selected
            .iter()
            .map(|member| member.display.clone())
            .collect();
        assert_eq!(names, ["paper", "slides"]);
    }

    #[test]
    fn select_members_selects_a_repeated_name_once() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = two_member_workspace(tmp.path());
        let selected =
            select_members(&workspace, &["paper".to_owned(), "paper".to_owned()]).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].display, "paper");
    }

    #[test]
    fn select_members_reports_unknown_names_with_the_available_list() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = two_member_workspace(tmp.path());
        let error = select_members(&workspace, &["ghost".to_owned()]).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            error.to_string(),
            "error: unknown project `ghost`: available: paper, slides"
        );
    }
}
