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

//! Configuration discovery and strict TOML parsing.
//!
//! [`discover`] locates `texmake.toml`: an explicit `--file` path wins,
//! otherwise `texmake.toml` is looked for in the working directory only.
//! [`load`] reads the file and parses it into
//! the span-carrying `Raw*` types here: every leaf value remembers where it
//! came from, so validation can later report `{path}:{line}:{column}` errors.
//! Unknown keys are rejected while parsing (`deny_unknown_fields`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;
use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
use toml::Spanned;

use crate::error::{ConfigError, Error};

/// The configuration file name texmake looks for in the working directory.
pub const FILE_NAME: &str = "texmake.toml";

/// A configuration file that was found, read and parsed successfully.
///
/// A file describes either one project (`[project]`) or a workspace root
/// (`[workspace]`); [`load`] decides which shape it has.
#[derive(Debug)]
pub enum LoadedConfig {
    /// A single project.
    Project(LoadedProject),
    /// A workspace root; its members are resolved by validation.
    Workspace(LoadedWorkspace),
}

/// The configuration file a project or workspace was read from.
#[derive(Debug)]
pub struct SourceFile {
    /// Absolute path of the configuration file.
    pub path: PathBuf,
    /// Root directory: the one that contains the configuration file.
    pub root: PathBuf,
    /// File contents; kept so spans can be turned into line/column pairs.
    pub source: String,
}

impl SourceFile {
    /// Build a semantic configuration error pointing at `span`.
    pub fn error_at(&self, span: std::ops::Range<usize>, reason: impl Into<String>) -> ConfigError {
        semantic(&self.path, &self.source, span, reason)
    }
}

/// A parsed project configuration.
#[derive(Debug)]
pub struct LoadedProject {
    /// The file it was read from.
    pub file: SourceFile,
    /// Parsed project configuration with source spans preserved.
    pub raw: RawProjectConfig,
}

/// A parsed workspace-root configuration.
#[derive(Debug)]
pub struct LoadedWorkspace {
    /// The file it was read from.
    pub file: SourceFile,
    /// The `[workspace]` table.
    pub workspace: RawWorkspace,
}

/// A `texmake.toml` that describes one project.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProjectConfig {
    /// The mandatory `[project]` table.
    pub project: RawProject,
    /// `[recipes.<name>]` tables; at least one is required semantically.
    #[serde(default)]
    pub recipes: BTreeMap<String, RawRecipe>,
    /// `[tools.<name>]` definitions, referenced by `tool` steps.
    #[serde(default)]
    pub tools: BTreeMap<String, Spanned<RawTool>>,
}

/// The `[workspace]` table of a workspace root.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWorkspace {
    /// Member directories, relative to the workspace root.
    pub members: Spanned<Vec<Spanned<String>>>,
}

/// The whole document, before the project/workspace shape is decided.
///
/// Both tables are optional here so their coexistence can be reported as a
/// positioned semantic error instead of a bare `toml` "missing field" error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    /// The `[project]` table, when present.
    project: Option<RawProject>,
    /// The `[workspace]` table, when present.
    workspace: Option<RawWorkspace>,
    /// `[recipes.<name>]` tables; only a project may define them.
    #[serde(default)]
    recipes: BTreeMap<String, RawRecipe>,
    /// `[tools.<name>]` definitions; only a project may define them.
    #[serde(default)]
    tools: BTreeMap<String, Spanned<RawTool>>,
}

/// The `[project]` table.
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRecipe {
    /// Recipe-level override of the project main document.
    pub main: Option<Spanned<String>>,
    /// Recipe-level override of the project output directory.
    pub output: Option<Spanned<String>>,
    /// Ordered build steps; at least one is required semantically.
    ///
    /// `None` when the key is absent, so an empty `steps = []` and a missing
    /// `steps` can both be reported; the array's own span is kept for that
    /// error.
    #[serde(default)]
    pub steps: Option<Spanned<Vec<Spanned<RawStep>>>>,
}

/// One step inside a recipe's `steps` array.
///
/// Exactly one action key must be present; that rule is checked semantically,
/// so every action key is optional here.
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
    ///
    /// `None` distinguishes a missing `args` key from `args = []`, which
    /// matters because `run` steps must not carry `args` at all.
    #[serde(default)]
    pub args: Option<Vec<Spanned<String>>>,
}

/// One `[tools.<name>]` definition.
///
/// A tool is either a bare command string or a table with `command` and
/// `args`. Both forms are deserialized through [`RawTool::deserialize`] so an
/// unknown key can still be reported with its source position; that is why the
/// tool definition is not a plain `#[derive(Deserialize)]` enum.
#[derive(Debug)]
pub enum RawTool {
    /// The string shorthand: the value is the command itself.
    Command(String),
    /// The table form. Both keys are validated semantically later.
    Table {
        /// Executable name or path.
        command: Option<Spanned<String>>,
        /// Fixed arguments; rendered like step arguments.
        args: Option<Vec<Spanned<String>>>,
    },
}

impl<'de> Deserialize<'de> for RawTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(RawToolVisitor)
    }
}

struct RawToolVisitor;

impl<'de> Visitor<'de> for RawToolVisitor {
    type Value = RawTool;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a command string or a table with `command` and `args`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<RawTool, E> {
        Ok(RawTool::Command(value.to_owned()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawTool, A::Error> {
        let mut command = None;
        let mut args = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "command" => command = Some(map.next_value::<Spanned<String>>()?),
                "args" => args = Some(map.next_value::<Vec<Spanned<String>>>()?),
                other => {
                    let message = format!("unknown key `{other}` in a tool definition");
                    return Err(map.next_value_seed(UnknownKey(message)).unwrap_err());
                }
            }
        }
        Ok(RawTool::Table { command, args })
    }
}

/// A [`DeserializeSeed`] that always fails with a fixed message.
///
/// Rejecting an unknown tool key this way keeps TOML's position information:
/// the surrounding map access attaches the rejected value's span to the error.
struct UnknownKey(String);

impl<'de> DeserializeSeed<'de> for UnknownKey {
    type Value = std::convert::Infallible;

    fn deserialize<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(de::Error::custom(self.0))
    }
}

/// Find the configuration file to use.
///
/// An explicit `--file` path is used as given (relative paths are resolved
/// against `cwd`) and must exist. Otherwise `texmake.toml` is looked for in
/// `cwd` only, never in a parent directory. `cwd` is normally absolute, which
/// makes the returned path absolute too.
pub fn discover(explicit: Option<&Path>, cwd: &Path) -> anyhow::Result<PathBuf> {
    if let Some(explicit) = explicit {
        let path = if explicit.is_absolute() {
            explicit.to_path_buf()
        } else {
            cwd.join(explicit)
        };
        if path.is_file() {
            return Ok(path);
        }
        bail!("configuration file not found: {}", path.display());
    }

    let path = cwd.join(FILE_NAME);
    if path.is_file() {
        return Ok(path);
    }

    bail!("no {FILE_NAME} found in {}", cwd.display())
}

/// Read and strictly parse the configuration file at `path`.
///
/// The returned [`LoadedConfig`] tells whether the file describes a project or
/// a workspace root; a file that mixes both tables, or defines neither, is a
/// positioned configuration error.
pub fn load(path: &Path) -> anyhow::Result<LoadedConfig> {
    let path = absolute(path);
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let file: RawFile = toml::from_str(&source).map_err(|source| {
        Error::Config(ConfigError::Toml {
            path: path.clone(),
            source,
        })
    })?;
    let root = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    let source_file = SourceFile { path, root, source };
    match (file.project, file.workspace) {
        (Some(_), Some(workspace)) => Err(Error::Config(source_file.error_at(
            workspace.members.span(),
            "a configuration cannot define both `[project]` and `[workspace]`",
        ))
        .into()),
        (None, None) => Err(Error::Config(source_file.error_at(
            0..0,
            "the configuration must define `[project]` or `[workspace]`",
        ))
        .into()),
        (Some(project), None) => Ok(LoadedConfig::Project(LoadedProject {
            file: source_file,
            raw: RawProjectConfig {
                project,
                recipes: file.recipes,
                tools: file.tools,
            },
        })),
        (None, Some(workspace)) => {
            if !file.recipes.is_empty() {
                return Err(Error::Config(source_file.error_at(
                    workspace.members.span(),
                    "a workspace root cannot define `recipes`",
                ))
                .into());
            }
            if !file.tools.is_empty() {
                return Err(Error::Config(source_file.error_at(
                    workspace.members.span(),
                    "a workspace root cannot define `tools`",
                ))
                .into());
            }
            Ok(LoadedConfig::Workspace(LoadedWorkspace {
                file: source_file,
                workspace,
            }))
        }
    }
}

/// Resolve `path` against the working directory when it is relative.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// Resolve `span` in `source` to a positioned [`ConfigError::Semantic`].
fn semantic(
    path: &Path,
    source: &str,
    span: std::ops::Range<usize>,
    reason: impl Into<String>,
) -> ConfigError {
    let (line, column) = line_column(source, span.start);
    ConfigError::Semantic {
        path: path.to_path_buf(),
        line,
        column,
        reason: reason.into(),
    }
}

/// Resolve a byte offset to a 1-based line and column.
fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(1, |prefix| prefix.chars().count() + 1);
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Load `path` and require a project configuration.
    fn load_project(path: &Path) -> LoadedProject {
        match load(path).unwrap() {
            LoadedConfig::Project(project) => project,
            LoadedConfig::Workspace(_) => panic!("expected a project configuration"),
        }
    }

    #[test]
    fn discovers_config_in_the_current_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("texmake.toml"), "").unwrap();
        assert_eq!(
            discover(None, tmp.path()).unwrap(),
            tmp.path().join("texmake.toml")
        );
    }

    #[test]
    fn ignores_config_in_a_parent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("texmake.toml"), "").unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let error = discover(None, &sub).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("no texmake.toml found in {}", sub.display())
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
    fn missing_explicit_file_reports_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let error = discover(Some(Path::new("nope.toml")), tmp.path()).unwrap_err();
        // The exit code (2) is covered end-to-end by
        // `tests/cli.rs::explicit_missing_config`; this unit test pins the
        // message contract without pulling the CLI layer in.
        assert_eq!(
            error.to_string(),
            format!(
                "configuration file not found: {}",
                tmp.path().join("nope.toml").display()
            )
        );
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

        let loaded = load_project(&path);
        assert_eq!(loaded.file.path, path);
        assert_eq!(loaded.file.root, tmp.path());
        assert_eq!(loaded.raw.project.name.get_ref(), "thesis");
        assert_eq!(
            loaded.raw.recipes["release"]
                .steps
                .as_ref()
                .unwrap()
                .get_ref()[0]
                .get_ref()
                .latex
                .as_ref()
                .unwrap()
                .get_ref(),
            "xelatex"
        );
        let span = loaded.raw.project.name.span();
        assert_eq!(&loaded.file.source[span], "\"thesis\"");
    }
    #[test]
    fn loads_tool_shorthand_and_table() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[tools]\npandoc = \"pandoc\"\n\n[tools.graphviz]\ncommand = \"dot\"\nargs = [\"-Tpdf\"]\n",
        )
        .unwrap();

        let loaded = load_project(&path);
        match loaded.raw.tools["pandoc"].get_ref() {
            RawTool::Command(command) => assert_eq!(command, "pandoc"),
            other => panic!("expected shorthand, got {other:?}"),
        }
        match loaded.raw.tools["graphviz"].get_ref() {
            RawTool::Table { command, args } => {
                assert_eq!(command.as_ref().unwrap().get_ref(), "dot");
                assert_eq!(args.as_ref().unwrap()[0].get_ref(), "-Tpdf");
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn tool_unknown_key_reports_position() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"t\"\nmain = \"main.tex\"\n[tools.pandoc]\nbogus = 1\n",
        )
        .unwrap();

        let error = load(&path).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("line 5"), "{text}");
        assert!(text.contains("unknown key `bogus`"), "{text}");
    }

    #[test]
    fn loads_a_workspace_root_with_member_spans() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(&path, "[workspace]\nmembers = [\"paper\", \"slides\"]\n").unwrap();

        let loaded = match load(&path).unwrap() {
            LoadedConfig::Workspace(loaded) => loaded,
            LoadedConfig::Project(_) => panic!("expected a workspace"),
        };
        assert_eq!(loaded.file.path, path);
        assert_eq!(loaded.file.root, tmp.path());
        let members = loaded.workspace.members.get_ref();
        assert_eq!(members[0].get_ref(), "paper");
        assert_eq!(&loaded.file.source[members[1].span()], "\"slides\"");
    }

    #[test]
    fn rejects_project_and_workspace_in_one_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"t\"\nmain = \"main.tex\"\n[workspace]\nmembers = []\n",
        )
        .unwrap();

        let text = load(&path).unwrap_err().to_string();
        assert!(
            text.contains("cannot define both `[project]` and `[workspace]`"),
            "{text}"
        );
        assert!(text.contains(&path.display().to_string()), "{text}");
    }

    #[test]
    fn rejects_recipes_in_a_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(
            &path,
            "[workspace]\nmembers = []\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
        )
        .unwrap();

        let text = load(&path).unwrap_err().to_string();
        assert!(
            text.contains("a workspace root cannot define `recipes`"),
            "{text}"
        );
    }

    #[test]
    fn rejects_a_file_without_project_or_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(&path, "").unwrap();

        let text = load(&path).unwrap_err().to_string();
        assert!(
            text.contains("must define `[project]` or `[workspace]`"),
            "{text}"
        );
    }

    #[test]
    fn error_at_reports_line_and_column() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("texmake.toml");
        std::fs::write(&path, "[project]\nname = \"thesis\"\nmain = \"main.tex\"\n").unwrap();

        let loaded = load_project(&path);
        let error = loaded
            .file
            .error_at(loaded.raw.project.name.span(), "bad name");
        assert_eq!(
            error.to_string(),
            format!("{}:2:8: error: bad name", path.display())
        );
    }
}
