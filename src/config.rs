//! Configuration discovery and strict TOML parsing.
//!
//! [`discover`] locates `texmake.toml`: an explicit `--file` path wins,
//! otherwise the file is searched for while walking up from the working
//! directory, nearest match first. [`load`] reads the file and parses it into
//! the span-carrying `Raw*` types here: every leaf value remembers where it
//! came from, so validation can later report `{path}:{line}:{column}` errors.
//! Unknown keys are rejected while parsing (`deny_unknown_fields`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Spanned;

use crate::error::{ConfigError, Error};

/// The file name texmake looks for while walking up from the working directory.
pub const FILE_NAME: &str = "texmake.toml";

/// A configuration file that was found, read and parsed successfully.
#[allow(dead_code)] // fields are consumed by validation in a later task
#[derive(Debug)]
pub struct LoadedConfig {
    /// Absolute path of the configuration file.
    pub path: PathBuf,
    /// Project root: the directory that contains the configuration file.
    pub root: PathBuf,
    /// File contents; kept so spans can be turned into line/column pairs.
    pub source: String,
    /// Parsed configuration with source spans preserved.
    pub raw: RawConfig,
}

/// The whole `texmake.toml` document.
#[allow(dead_code)] // fields are consumed by validation in a later task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    /// The mandatory `[project]` table.
    pub project: RawProject,
    /// `[recipes.<name>]` tables; at least one is required semantically.
    #[serde(default)]
    pub recipes: BTreeMap<String, RawRecipe>,
}

/// The `[project]` table.
#[allow(dead_code)] // fields are consumed by validation in a later task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProject {
    /// Project name.
    pub name: Spanned<String>,
    /// Main document, relative to the project root.
    pub main: Spanned<String>,
    /// Output directory; defaults to `build`.
    pub output: Option<Spanned<String>>,
    /// Default recipe; optional.
    pub default: Option<Spanned<String>>,
}

/// One `[recipes.<name>]` table.
#[allow(dead_code)] // fields are consumed by validation in a later task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRecipe {
    /// Recipe-level override of the project main document.
    pub main: Option<Spanned<String>>,
    /// Recipe-level override of the project output directory.
    pub output: Option<Spanned<String>>,
    /// Ordered build steps; at least one is required semantically.
    #[serde(default)]
    pub steps: Vec<Spanned<RawStep>>,
}

/// One step inside a recipe's `steps` array.
///
/// Exactly one action key must be present; that rule is checked semantically,
/// so every action key is optional here.
#[allow(dead_code)] // fields are consumed by validation in a later task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStep {
    /// `{ latex = "xelatex" }`.
    pub latex: Option<Spanned<String>>,
    /// `{ biber = true }`.
    pub biber: Option<Spanned<bool>>,
    /// `{ bibtex = true }`.
    pub bibtex: Option<Spanned<bool>>,
    /// `{ makeindex = true }`.
    pub makeindex: Option<Spanned<bool>>,
    /// `{ run = "sh scripts/gen.sh" }`.
    pub run: Option<Spanned<String>>,
    /// `{ tool = "graphviz" }`.
    pub tool: Option<Spanned<String>>,
    /// Extra arguments appended to the built-in command.
    #[serde(default)]
    pub args: Vec<Spanned<String>>,
}

/// Find the configuration file to use.
///
/// An explicit `--file` path is used as given (relative paths are resolved
/// against `cwd`) and must exist. Otherwise `texmake.toml` is searched for
/// from `cwd` upward. `cwd` is normally absolute, which makes the returned
/// path absolute too.
pub fn discover(explicit: Option<&Path>, cwd: &Path) -> Result<PathBuf, Error> {
    if let Some(explicit) = explicit {
        let path = if explicit.is_absolute() {
            explicit.to_path_buf()
        } else {
            cwd.join(explicit)
        };
        if path.is_file() {
            return Ok(path);
        }
        return Err(Error::Environment(format!(
            "configuration file not found: {}",
            path.display()
        )));
    }

    for directory in cwd.ancestors() {
        let candidate = directory.join(FILE_NAME);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(Error::Environment(format!(
        "no {FILE_NAME} found from {} upward",
        cwd.display()
    )))
}

/// Read and strictly parse the configuration file at `path`.
pub fn load(path: &Path) -> Result<LoadedConfig, Error> {
    let path = absolute(path);
    let source = std::fs::read_to_string(&path)
        .map_err(|error| Error::Environment(format!("cannot read {}: {error}", path.display())))?;
    let raw: RawConfig = toml::from_str(&source).map_err(|source| ConfigError::Toml {
        path: path.clone(),
        source,
    })?;
    let root = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    Ok(LoadedConfig {
        path,
        root,
        source,
        raw,
    })
}

/// Resolve `path` against the working directory when it is relative.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_config_in_parent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("texmake.toml"), "").unwrap();
        let sub = tmp.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(
            discover(None, &sub).unwrap(),
            tmp.path().join("texmake.toml")
        );
    }

    #[test]
    fn uses_explicit_file_and_resolves_relative_to_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("cfg")).unwrap();
        std::fs::write(tmp.path().join("cfg/texmake.toml"), "").unwrap();
        assert_eq!(
            discover(Some(Path::new("cfg/texmake.toml")), tmp.path()).unwrap(),
            tmp.path().join("cfg/texmake.toml")
        );
    }

    #[test]
    fn missing_explicit_file_is_environment_error() {
        let tmp = tempfile::tempdir().unwrap();
        let error = discover(Some(Path::new("nope.toml")), tmp.path()).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("nope.toml"), "{error}");
    }

    #[test]
    fn load_sets_root_and_keeps_spans() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"thesis\"\nmain = \"main.tex\"\n\n[recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n",
        )
        .unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.path, path);
        assert_eq!(loaded.root, tmp.path());
        assert_eq!(loaded.raw.project.name.get_ref(), "thesis");
        assert_eq!(
            loaded.raw.recipes["release"].steps[0]
                .get_ref()
                .latex
                .as_ref()
                .unwrap()
                .get_ref(),
            "xelatex"
        );
        let span = loaded.raw.project.name.span();
        assert_eq!(&loaded.source[span], "\"thesis\"");
    }
}
