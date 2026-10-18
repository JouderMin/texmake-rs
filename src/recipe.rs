//! Recipe selection and step expansion.
//!
//! [`resolve_steps`] turns the validated [`Step`] list into concrete argv
//! commands. Nothing here touches the filesystem or checks for tools: that is
//! the build's job, so `--dry-run` can print the exact command lines without
//! side effects.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::validate::{Project, Recipe, Step};

/// One step with a printable command line and the argv that runs it.
pub struct ResolvedStep {
    /// Command line shown by `--dry-run` and by failure reports.
    pub display: String,
    /// Execution details read by the build.
    pub exec: ExecSpec,
}

/// How to execute one resolved step without a shell (except `run` steps).
pub struct ExecSpec {
    /// Executable to spawn.
    ///
    /// Built-in steps borrow a `'static` name; custom tools own theirs.
    pub program: Cow<'static, str>,
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
            let mut argv = Vec::with_capacity(args.len() + 5);
            argv.push("-interaction=nonstopmode".to_owned());
            argv.push("-halt-on-error".to_owned());
            argv.push("-file-line-error".to_owned());
            argv.push(format!("-output-directory={}", recipe.output));
            argv.extend(args.iter().cloned());
            argv.push(recipe.main.clone());
            executable(
                Cow::Borrowed(engine.command()),
                argv,
                root.to_path_buf(),
                true,
            )
        }
        Step::Biber { args } => {
            let mut argv = Vec::with_capacity(args.len() + 1);
            argv.extend(args.iter().cloned());
            argv.push(format!("{}/{}", recipe.output, recipe.main_stem));
            executable(Cow::Borrowed("biber"), argv, root.to_path_buf(), true)
        }
        Step::Bibtex { args } => {
            let mut argv = Vec::with_capacity(args.len() + 1);
            argv.extend(args.iter().cloned());
            argv.push(format!("{}/{}", recipe.output, recipe.main_stem));
            executable(Cow::Borrowed("bibtex"), argv, root.to_path_buf(), true)
        }
        Step::Makeindex { args } => {
            let mut argv = Vec::with_capacity(args.len() + 1);
            argv.extend(args.iter().cloned());
            argv.push(recipe.main_stem.clone());
            executable(
                Cow::Borrowed("makeindex"),
                argv,
                recipe.output_path.clone(),
                true,
            )
        }
        Step::Tool { command, args } => executable(
            Cow::Owned(command.clone()),
            args.clone(),
            root.to_path_buf(),
            true,
        ),
        Step::Run { command } => {
            let (program, flag) = shell();
            ResolvedStep {
                display: command.clone(),
                exec: ExecSpec {
                    program: Cow::Borrowed(program),
                    args: vec![flag.to_owned(), command.clone()],
                    cwd: root.to_path_buf(),
                    check_tool: false,
                },
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
fn executable(
    program: Cow<'static, str>,
    args: Vec<String>,
    cwd: PathBuf,
    check_tool: bool,
) -> ResolvedStep {
    let display = display_command(program.as_ref(), &args);
    ResolvedStep {
        display,
        exec: ExecSpec {
            program,
            args,
            cwd,
            check_tool,
        },
    }
}

/// Render one command line the way `--dry-run` prints it: `argv` joined with
/// spaces, each element quoted when needed.
fn display_command(program: &str, args: &[String]) -> String {
    let mut display = String::with_capacity(
        program.len() + args.iter().map(|arg| arg.len() + 2).sum::<usize>() + 1,
    );
    quote_into(program, &mut display);
    for arg in args {
        display.push(' ');
        quote_into(arg, &mut display);
    }
    display
}

/// Append one shell-quoted argument.
///
/// Quotes are added only when the argument is empty or contains whitespace or a
/// quote. Inside the quotes, `"` and `\` are escaped with a backslash.
fn quote_into(argument: &str, quoted: &mut String) {
    let needs_quotes = argument.is_empty()
        || argument
            .chars()
            .any(|character| character.is_whitespace() || character == '"' || character == '\'');
    if !needs_quotes {
        quoted.push_str(argument);
        return;
    }
    quoted.push('"');
    for character in argument.chars() {
        if character == '"' || character == '\\' {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
}

#[cfg(test)]
fn quote(argument: &str) -> String {
    let mut quoted = String::new();
    quote_into(argument, &mut quoted);
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
