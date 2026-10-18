//! Project scaffolding for `texmake init`.
//!
//! `init` is the only command that writes into the project root: it creates a
//! minimal `texmake.toml`, a compilable `main.tex` and a `.gitignore`. Existing
//! files are never overwritten; an existing configuration aborts the whole
//! operation so a project is never half-initialized.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::ValueEnum;

use crate::error::Error;

/// LaTeX engine recorded in the generated recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Engine {
    Pdflatex,
    Xelatex,
    Lualatex,
}

impl Engine {
    /// The engine name as written to `texmake.toml`.
    fn name(self) -> &'static str {
        match self {
            Engine::Pdflatex => "pdflatex",
            Engine::Xelatex => "xelatex",
            Engine::Lualatex => "lualatex",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

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
}

/// Create a new document project in `opts.dir`.
///
/// The target directory is created if needed. `texmake.toml` is never
/// overwritten: if it already exists the command fails before writing anything.
/// Other files that already exist are kept and reported as skipped.
pub fn run_init(opts: &InitOptions) -> Result<(), Error> {
    if opts.engine == Engine::Pdflatex && opts.language == Language::Zh {
        return Err(Error::Usage(
            "`pdflatex` cannot typeset Chinese; use `--language en` or another engine".to_owned(),
        ));
    }

    let dir = target_dir(&opts.dir)?;
    let name = project_name(&dir, opts.name.as_deref())?;

    std::fs::create_dir_all(&dir).map_err(|error| {
        Error::Environment(format!("cannot create `{}`: {error}", dir.display()))
    })?;

    // `create_new` makes the refusal race-free: a configuration that appears
    // between the check and the write is still refused, not overwritten.
    let config_path = dir.join("texmake.toml");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config_path)
    {
        Ok(mut file) => {
            write_file(&mut file, &config_template(&name, opts.engine))?;
            println!("created texmake.toml");
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::Usage(format!(
                "refusing to overwrite `{}`",
                config_path.display()
            )));
        }
        Err(error) => {
            return Err(Error::Environment(format!(
                "cannot create `{}`: {error}",
                config_path.display()
            )));
        }
    }

    let files = [
        ("main.tex", main_template(&name, opts.language)),
        (".gitignore", "/build/\n".to_owned()),
    ];
    for (file, content) in files {
        let path = dir.join(file);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut handle) => {
                write_file(&mut handle, &content)?;
                println!("created {file}");
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                println!("skipped existing {file}");
            }
            Err(error) => {
                return Err(Error::Environment(format!(
                    "cannot create `{}`: {error}",
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

/// The directory to initialize: `.` and an empty path mean the current
/// directory, so the project name can be derived from its real name.
fn target_dir(dir: &Path) -> Result<PathBuf, Error> {
    if dir.as_os_str().is_empty() || dir == Path::new(".") {
        return std::env::current_dir().map_err(|error| {
            Error::Environment(format!("cannot read working directory: {error}"))
        });
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

fn write_file(file: &mut std::fs::File, content: &str) -> Result<(), Error> {
    file.write_all(content.as_bytes())
        .map_err(|error| Error::Environment(format!("cannot write project file: {error}")))
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
fn main_template(name: &str, language: Language) -> String {
    let (class, body) = match language {
        Language::Zh => ("ctexart", "你好，世界。"),
        Language::En => ("article", "Hello, world."),
    };
    format!(
        "\\documentclass{{{class}}}
\\title{{{}}}
\\begin{{document}}
\\maketitle
{body}
\\end{{document}}
",
        escape_latex(name)
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

/// Escape LaTeX's special characters so any directory name is typesettable.
fn escape_latex(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len());
    for character in name.chars() {
        match character {
            '\\' => escaped.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '~' => escaped.push_str("\\textasciitilde{}"),
            '^' => escaped.push_str("\\textasciicircum{}"),
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
        let zh = main_template("论文", Language::Zh);
        assert!(zh.contains("\\documentclass{ctexart}"));
        assert!(zh.contains("你好，世界。"));
        assert!(zh.contains("\\title{论文}"));

        let en = main_template("paper", Language::En);
        assert!(en.contains("\\documentclass{article}"));
        assert!(en.contains("Hello, world."));
    }

    #[test]
    fn escapes_latex_specials_in_the_title() {
        let main = main_template("a_b&c", Language::En);
        assert!(main.contains("\\title{a\\_b\\&c}"), "{main}");
    }
}
