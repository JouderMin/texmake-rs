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

//! Project scaffolding for `texmake init`.
//!
//! `init` is the only command that writes into the project root: it creates a
//! minimal `texmake.toml`, a compilable `main.tex` and a `.gitignore`. Existing
//! files are never overwritten; an existing configuration aborts the operation
//! before anything is written. The document files are written before the
//! configuration, and a write failure removes the files created by that run, so
//! a failed run does not leave a configuration (or other new files) behind.
//! With `dry_run` nothing is created and the planned actions are printed
//! instead; `quiet` suppresses all progress lines.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::ValueEnum;

use crate::engine::Engine;
use crate::error::{Error, usage};

/// Document language of the generated `main.tex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Language {
    Zh,
    En,
}

impl Language {
    fn name(self) -> &'static str {
        match self {
            Language::Zh => "zh",
            Language::En => "en",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Everything `texmake init` needs, already parsed from the command line.
#[derive(Debug)]
pub struct InitOptions {
    /// Target directory; `.` means the current directory.
    pub dir: PathBuf,
    /// Project name; defaults to the target directory's name.
    pub name: Option<String>,
    /// LaTeX engine used by the generated recipe.
    pub engine: Engine,
    /// Document language of the generated `main.tex`.
    pub language: Language,
    /// Only report what would be created; write nothing.
    pub dry_run: bool,
    /// Suppress progress lines (`created`, `skipped`, `would ...`).
    pub quiet: bool,
}

/// Create a new document project in `opts.dir`.
///
/// The target directory is created if needed. `texmake.toml` is never
/// overwritten: if it already exists the command fails before writing anything.
/// Other files that already exist are kept and reported as skipped. With
/// `dry_run` no directory or file is created; the planned actions are printed
/// instead. `quiet` suppresses the progress lines.
pub fn run_init(opts: &InitOptions) -> anyhow::Result<()> {
    if opts.engine == Engine::Pdflatex {
        return Err(usage(
            "`pdflatex` is not supported: the generated template uses `ctex` and `fontsetup`, which need `xelatex` or `lualatex`".to_owned(),
        ));
    }

    let dir = target_dir(&opts.dir)?;
    let name = project_name(&dir, opts.name.as_deref())?;

    let config_path = dir.join("texmake.toml");
    let main_path = dir.join("main.tex");
    let gitignore_path = dir.join(".gitignore");
    precheck_targets(&config_path, &main_path, &gitignore_path)?;

    if opts.dry_run {
        for (path, label) in [
            (&config_path, "texmake.toml"),
            (&main_path, "main.tex"),
            (&gitignore_path, ".gitignore"),
        ] {
            if opts.quiet {
                continue;
            }
            if path.exists() {
                println!("would skip existing {label}");
            } else {
                println!("would create {label}");
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create `{}`", dir.display()))?;

    write_project(&dir, &name, opts, &write_new_file)
}

/// Refuse targets that cannot be written as regular files, before the target
/// directory or any file is created. An existing configuration is refused as a
/// whole; other existing regular files are kept and skipped later.
fn precheck_targets(config: &Path, main: &Path, gitignore: &Path) -> anyhow::Result<()> {
    for (path, is_config) in [(config, true), (main, false), (gitignore, false)] {
        let shown = path.display();
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(usage(format!(
                    "refusing to initialize: `{shown}` exists and is not a regular file"
                )));
            }
            Ok(_) if is_config => {
                return Err(usage(format!("refusing to overwrite `{shown}`")));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("cannot inspect `{shown}`"));
            }
        }
    }
    Ok(())
}

/// Write `main.tex`, `.gitignore` and finally `texmake.toml` into `dir`.
///
/// `writer` is a parameter so tests can inject a failing write. Files created by
/// this call are removed again when a later write fails; files that already
/// existed are never touched.
fn write_project(
    dir: &Path,
    name: &str,
    opts: &InitOptions,
    writer: &dyn Fn(&Path, &str) -> io::Result<()>,
) -> anyhow::Result<()> {
    let config_path = dir.join("texmake.toml");
    let files = [
        (
            dir.join("main.tex"),
            "main.tex",
            main_template(opts.language),
        ),
        (dir.join(".gitignore"), ".gitignore", "/build/\n".to_owned()),
    ];

    let mut created: Vec<PathBuf> = Vec::new();
    let outcome = (|| -> anyhow::Result<()> {
        for (path, label, content) in &files {
            match writer(path, content) {
                Ok(()) => {
                    created.push(path.clone());
                    if !opts.quiet {
                        println!("created {label}");
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if !opts.quiet {
                        println!("skipped existing {label}");
                    }
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("cannot create `{}`", path.display()));
                }
            }
        }

        // The configuration goes last: a failure above never leaves a
        // configuration without its document files.
        match writer(&config_path, &config_template(name, opts.engine)) {
            Ok(()) => {
                created.push(config_path.clone());
                if !opts.quiet {
                    println!("created texmake.toml");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(usage(format!(
                    "refusing to overwrite `{}`",
                    config_path.display()
                )));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot create `{}`", config_path.display()));
            }
        }
        Ok(())
    })();

    if let Err(error) = outcome {
        for path in created.iter().rev() {
            let _ = std::fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(())
}

/// Create a file that must not exist yet and write `content` into it.
fn write_new_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(content.as_bytes())
}

/// The directory to initialize: `.` and an empty path mean the current
/// directory, so the project name can be derived from its real name.
fn target_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    if dir.as_os_str().is_empty() || dir == Path::new(".") {
        return std::env::current_dir().context("cannot read working directory");
    }
    Ok(dir.to_path_buf())
}

/// The project name: `--name` wins, otherwise the target directory's name.
fn project_name(dir: &Path, name: Option<&str>) -> Result<String, Error> {
    if let Some(name) = name {
        return Ok(name.to_owned());
    }
    dir.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::Usage(format!(
                "cannot derive a project name from `{}`; pass `--name`",
                dir.display()
            ))
        })
}

/// The generated `texmake.toml`.
fn config_template(name: &str, engine: Engine) -> String {
    format!(
        "[project]
name = \"{}\"
main = \"main.tex\"
output = \"build\"
default = \"release\"

[recipes.release]
steps = [
  {{ latex = \"{}\" }},
]
",
        escape_toml(name),
        engine
    )
}

/// The generated `main.tex`.
fn main_template(language: Language) -> String {
    let preamble = match language {
        Language::Zh => "\\usepackage[UTF8]{ctex}\n\\usepackage[default]{fontsetup}",
        Language::En => "\\usepackage[default]{fontsetup}",
    };
    let body = match language {
        Language::Zh => "你好，世界。",
        Language::En => "Hello, world.",
    };
    format!(
        "\\documentclass{{article}}
{preamble}
\\begin{{document}}
{body}
\\end{{document}}
"
    )
}

/// Escape `name` for a TOML basic string.
fn escape_toml(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for character in name.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_engine_and_name() {
        let config = config_template("thesis", Engine::Lualatex);
        assert!(config.contains("name = \"thesis\""));
        assert!(config.contains("{ latex = \"lualatex\" }"));
        assert!(config.contains("default = \"release\""));
    }

    #[test]
    fn escapes_quotes_in_the_name() {
        assert!(config_template("a\"b", Engine::Xelatex).contains("name = \"a\\\"b\""));
        assert_eq!(escape_toml("a\\b"), "a\\\\b");
    }

    #[test]
    fn writes_language_specific_documents() {
        let zh = main_template(Language::Zh);
        assert!(zh.contains("\\documentclass{article}"));
        assert!(zh.contains("\\usepackage[UTF8]{ctex}"));
        assert!(zh.contains("\\usepackage[default]{fontsetup}"));
        assert!(zh.contains("你好，世界。"));
        assert!(!zh.contains("ctexart"));

        let en = main_template(Language::En);
        assert!(en.contains("\\documentclass{article}"));
        assert!(en.contains("\\usepackage[default]{fontsetup}"));
        assert!(en.contains("Hello, world."));
        assert!(!en.contains("ctex"));
    }

    fn quiet_options(dir: &Path) -> InitOptions {
        InitOptions {
            dir: dir.to_path_buf(),
            name: Some("t".to_owned()),
            engine: Engine::Xelatex,
            language: Language::En,
            dry_run: false,
            quiet: true,
        }
    }

    #[test]
    fn failed_config_write_rolls_back_created_files() {
        let dir = tempfile::tempdir().unwrap();
        let opts = quiet_options(dir.path());
        let writer = |path: &Path, content: &str| {
            if path.ends_with("texmake.toml") {
                return Err(io::Error::other("disk full"));
            }
            write_new_file(path, content)
        };
        let error = write_project(dir.path(), "t", &opts, &writer).unwrap_err();
        assert!(error.to_string().contains("texmake.toml"), "{error}");
        assert!(!dir.path().join("texmake.toml").exists());
        assert!(!dir.path().join("main.tex").exists());
        assert!(!dir.path().join(".gitignore").exists());
    }

    #[test]
    fn failed_config_write_keeps_files_that_already_existed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.tex"), "keep-me").unwrap();
        let opts = quiet_options(dir.path());
        let writer = |path: &Path, content: &str| {
            if path.ends_with("texmake.toml") {
                return Err(io::Error::other("disk full"));
            }
            write_new_file(path, content)
        };
        assert!(write_project(dir.path(), "t", &opts, &writer).is_err());
        assert!(!dir.path().join("texmake.toml").exists());
        assert!(!dir.path().join(".gitignore").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("main.tex")).unwrap(),
            "keep-me"
        );
    }
}
