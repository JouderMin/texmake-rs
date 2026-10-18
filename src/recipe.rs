//! Recipe selection and step expansion.
//!
//! [`resolve_steps`] turns the validated [`Step`] list into concrete argv
//! commands. Nothing here touches the filesystem or checks for tools: that is
//! the build's job, so `--dry-run` can print the exact command lines without
//! side effects.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::validate::{Project, Recipe, Step};

/// One step with a printable command line and, when executable, its argv.
pub struct ResolvedStep {
    /// Command line shown by `--dry-run` and by failure reports.
    pub display: String,
    /// Execution details, read when builds execute steps.
    #[allow(dead_code)] // read when builds execute steps in a later task
    pub exec: Option<ExecSpec>,
}

/// How to execute one resolved step without a shell (except `run` steps).
#[allow(dead_code)] // fields are read when builds execute steps in a later task
pub struct ExecSpec {
    /// Executable to spawn.
    pub program: String,
    /// Arguments passed verbatim.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Whether the build should check that `program` exists and is executable.
    pub check_tool: bool,
}

/// Pick the recipe named `name`, the configured default, or the only recipe.
///
/// # Errors
///
/// Returns [`Error::Usage`] when the named recipe does not exist, when several
/// recipes exist without a default, and when a name is required.
pub fn select_recipe<'a>(project: &'a Project, name: Option<&str>) -> Result<&'a Recipe, Error> {
    if let Some(name) = name {
        return project.recipes.get(name).ok_or_else(|| {
            Error::Usage(format!(
                "unknown recipe `{name}`; available recipes: {}",
                recipe_names(project)
            ))
        });
    }
    if let Some(default) = &project.default {
        return Ok(project
            .recipes
            .get(default)
            .expect("the default recipe is validated before selection"));
    }
    if project.recipes.len() == 1 {
        return Ok(project
            .recipes
            .values()
            .next()
            .expect("exactly one recipe is present"));
    }
    Err(Error::Usage(format!(
        "no default recipe is configured; name one of: {}",
        recipe_names(project)
    )))
}

fn recipe_names(project: &Project) -> String {
    project
        .recipes
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Expand `recipe`'s steps into the commands the build will run, in order.
pub fn resolve_steps(project: &Project, recipe: &Recipe) -> Vec<ResolvedStep> {
    recipe
        .steps
        .iter()
        .map(|step| resolve_step(&project.root, recipe, step))
        .collect()
}

fn resolve_step(root: &Path, recipe: &Recipe, step: &Step) -> ResolvedStep {
    match step {
        Step::Latex { engine, args } => {
            let mut argv = vec![
                "-interaction=nonstopmode".to_owned(),
                "-halt-on-error".to_owned(),
                "-file-line-error".to_owned(),
                format!("-output-directory={}", recipe.output),
            ];
            argv.extend(args.iter().cloned());
            argv.push(recipe.main.clone());
            executable(engine.command().to_owned(), argv, root.to_path_buf(), true)
        }
        Step::Biber { args } => {
            let mut argv = args.clone();
            argv.push(format!("{}/{}", recipe.output, recipe.main_stem));
            executable("biber".to_owned(), argv, root.to_path_buf(), true)
        }
        Step::Bibtex { args } => {
            let mut argv = args.clone();
            argv.push(format!("{}/{}", recipe.output, recipe.main_stem));
            executable("bibtex".to_owned(), argv, root.to_path_buf(), true)
        }
        Step::Makeindex { args } => {
            let mut argv = args.clone();
            argv.push(recipe.main_stem.clone());
            executable(
                "makeindex".to_owned(),
                argv,
                recipe.output_path.clone(),
                true,
            )
        }
        Step::Tool { command, args } => {
            executable(command.clone(), args.clone(), root.to_path_buf(), true)
        }
        Step::Run { command } => {
            let (program, flag) = shell();
            ResolvedStep {
                display: command.clone(),
                exec: Some(ExecSpec {
                    program: program.to_owned(),
                    args: vec![flag.to_owned(), command.clone()],
                    cwd: root.to_path_buf(),
                    check_tool: false,
                }),
            }
        }
    }
}

/// The system shell and the flag that makes it run a command string.
fn shell() -> (&'static str, &'static str) {
    #[cfg(windows)]
    {
        ("cmd", "/C")
    }
    #[cfg(not(windows))]
    {
        ("sh", "-c")
    }
}

/// Build a resolved step that runs `program` with `args` directly.
fn executable(program: String, args: Vec<String>, cwd: PathBuf, check_tool: bool) -> ResolvedStep {
    let display = display_command(&program, &args);
    ResolvedStep {
        display,
        exec: Some(ExecSpec {
            program,
            args,
            cwd,
            check_tool,
        }),
    }
}

/// Render one command line the way `--dry-run` prints it: `argv` joined with
/// spaces, each element quoted when needed.
fn display_command(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quote one argument when it is empty or contains whitespace or a quote.
///
/// Inside the quotes, `"` and `\` are escaped with a backslash.
fn quote(argument: &str) -> String {
    let needs_quotes = argument.is_empty()
        || argument
            .chars()
            .any(|character| character.is_whitespace() || character == '"' || character == '\'');
    if !needs_quotes {
        return argument.to_owned();
    }
    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    for character in argument.chars() {
        if character == '"' || character == '\\' {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(quote("plain"), "plain");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("a\\b"), "a\\b");
    }
}
