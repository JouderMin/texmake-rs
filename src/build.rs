//! Build orchestration.
//!
//! [`run_build`] creates the output directory, truncates the build log, checks
//! that the executable of every checked step is available, and then runs the
//! recipe's steps in order. A step's standard output and standard error are
//! merged into the log, which records the rendered command, the output and the
//! exit status. The first failing step stops the build and produces a failure
//! report with the command, the exit status, the last lines of output and the
//! log path.

use anyhow::Context;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::error::{BuildFailed, Error};
use crate::recipe::{self, ExecSpec, ResolvedStep};
use crate::up_to_date;
use crate::validate::{Project, Recipe};

/// Name of the build log inside the output directory.
const LOG_NAME: &str = "texmake.log";
/// Number of trailing output lines repeated by a failure report.
const TAIL_LINES: usize = 20;

/// Options controlling one build, taken from the global CLI flags.
pub struct BuildOptions {
    /// Ignore the up-to-date check.
    pub force: bool,
    /// Print commands without executing them.
    pub dry_run: bool,
    /// Print full commands and tool output.
    pub verbose: bool,
    /// Print errors only.
    pub quiet: bool,
}

/// Build `recipe`, or print its commands when `opts.dry_run` is set.
///
/// `--dry-run` prints every command even with `--quiet` and performs no
/// up-to-date check, no output-directory creation and no tool check.
///
/// `--verbose` prints each rendered command and every line of the steps'
/// merged output to stdout as it arrives; the log still records the same
/// lines.
///
/// # Errors
///
/// Returns an environment error when the output directory or the log cannot be
/// written or a step cannot be started, a tool error when a checked executable
/// is missing, and a build error when a step exits unsuccessfully.
pub fn run_build(project: &Project, recipe: &Recipe, opts: &BuildOptions) -> anyhow::Result<()> {
    if opts.dry_run {
        for step in recipe::resolve_steps(project, recipe) {
            println!("{}", step.display);
        }
        return Ok(());
    }
    if !opts.force && up_to_date::is_up_to_date(project, recipe) {
        if !opts.quiet {
            println!("Everything is up to date.");
        }
        return Ok(());
    }
    std::fs::create_dir_all(&recipe.output_path)
        .with_context(|| format!("cannot create `{}`", recipe.output_path.display()))?;
    let log_path = recipe.output_path.join(LOG_NAME);
    let mut log = File::create(&log_path)
        .with_context(|| format!("cannot write `{}`", log_path.display()))?;
    let steps = recipe::resolve_steps(project, recipe);
    check_tools(project, &steps)?;
    for step in &steps {
        // Every resolved step currently carries a command; the option leaves
        // room for steps that only exist to be printed.
        let Some(exec) = &step.exec else {
            continue;
        };
        let program = spawn_program(&project.root, exec);
        run_step(&mut log, &log_path, step, exec, &program, opts.verbose)?;
    }
    if !opts.quiet {
        println!("Built {}/{}.pdf", recipe.output, recipe.main_stem);
    }
    Ok(())
}

/// Resolve `program` to an existing executable file.
///
/// A program containing a path separator is taken relative to `root` (or used
/// as is when absolute); any other name is looked up in `PATH`. On Windows,
/// bare names are also matched against `PATHEXT`. Returns `None` when nothing
/// matching is found or the match is not executable.
pub fn resolve_program(root: &Path, program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    if contains_separator(program) {
        let path = Path::new(program);
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        return is_executable(&candidate).then_some(candidate);
    }
    let path_variable = std::env::var_os("PATH")?;
    std::env::split_paths(&path_variable)
        .find_map(|directory| find_in_directory(&directory, program))
}

/// Whether `program` names a path rather than a bare file name.
fn contains_separator(program: &str) -> bool {
    #[cfg(windows)]
    {
        program.contains('/') || program.contains('\\')
    }
    #[cfg(not(windows))]
    {
        program.contains('/')
    }
}

/// Look for the executable `program` in one `PATH` directory.
#[cfg(not(windows))]
fn find_in_directory(directory: &Path, program: &str) -> Option<PathBuf> {
    let candidate = directory.join(program);
    is_executable(&candidate).then_some(candidate)
}

/// Look for the executable `program` in one `PATH` directory.
///
/// A name without an extension is tried with every `PATHEXT` extension, so
/// `dot` finds `dot.exe`.
#[cfg(windows)]
fn find_in_directory(directory: &Path, program: &str) -> Option<PathBuf> {
    if Path::new(program).extension().is_some() {
        let candidate = directory.join(program);
        return is_executable(&candidate).then_some(candidate);
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    extensions
        .split(';')
        .filter(|extension| !extension.is_empty())
        .find_map(|extension| {
            let candidate = directory.join(format!("{program}{extension}"));
            is_executable(&candidate).then_some(candidate)
        })
}

/// Whether `path` is a regular file with execute permission.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Whether `path` is a regular file.
#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// The executable the build spawns for `exec`.
///
/// Checked steps spawn the resolved path, so the executable that was checked is
/// the executable that runs. `run` steps keep their shell name and let the
/// operating system search `PATH` when spawning.
fn spawn_program(root: &Path, exec: &ExecSpec) -> PathBuf {
    if exec.check_tool
        && let Some(path) = resolve_program(root, &exec.program)
    {
        return path;
    }
    PathBuf::from(&exec.program)
}

/// Fail with every missing executable of the checked steps, in step order.
fn check_tools(project: &Project, steps: &[ResolvedStep]) -> anyhow::Result<()> {
    let mut missing: Vec<String> = Vec::new();
    for step in steps {
        let Some(exec) = &step.exec else {
            continue;
        };
        if !exec.check_tool || missing.contains(&exec.program) {
            continue;
        }
        if resolve_program(&project.root, &exec.program).is_none() {
            missing.push(exec.program.clone());
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::ToolMissing(missing).into())
    }
}

/// Run one step, merging its output into the log and reporting a failure.
///
/// With `verbose`, the rendered command and every output line are also printed
/// to stdout as soon as they arrive, so the log and the terminal see the same
/// merged stream.
fn run_step(
    log: &mut File,
    log_path: &Path,
    step: &ResolvedStep,
    exec: &ExecSpec,
    program: &Path,
    verbose: bool,
) -> anyhow::Result<()> {
    let command = format!("$ {}", step.display);
    write_log(log, log_path, &command)?;
    if verbose {
        stream(&command);
    }
    let mut child = Command::new(program)
        .args(&exec.args)
        .current_dir(&exec.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start `{}`", exec.program))?;
    let stdout = child.stdout.take().expect("the child's stdout is piped");
    let stderr = child.stderr.take().expect("the child's stderr is piped");
    let (sender, receiver) = mpsc::channel::<String>();
    let stdout_reader = forward_lines(stdout, sender.clone());
    let stderr_reader = forward_lines(stderr, sender);
    let mut tail = VecDeque::with_capacity(TAIL_LINES);
    for line in receiver {
        write_log(log, log_path, &line)?;
        if verbose {
            stream(&line);
        }
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    stdout_reader
        .join()
        .expect("a line-forwarding thread never panics");
    stderr_reader
        .join()
        .expect("a line-forwarding thread never panics");
    let status = child
        .wait()
        .with_context(|| format!("cannot wait for `{}`", exec.program))?;
    let (exit_code, signal) = status_parts(status);
    write_log(log, log_path, &format_exit_status(exit_code, signal))?;
    if status.success() {
        return Ok(());
    }
    Err(Error::BuildFailed(BuildFailed {
        command: step.display.clone(),
        exit_code,
        signal,
        tail: tail.into_iter().collect(),
        log: log_path.to_path_buf(),
    })
    .into())
}

/// Send every line read from `reader` to `sender`.
///
/// Lines are split on `\n`; a trailing `\r` is dropped so Windows output does
/// not leak carriage returns into the log, and invalid UTF-8 is replaced rather
/// than treated as an error.
fn forward_lines<R>(reader: R, sender: mpsc::Sender<String>) -> thread::JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    while buffer
                        .last()
                        .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
                    {
                        buffer.pop();
                    }
                    let line = String::from_utf8_lossy(&buffer).into_owned();
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

/// Append one line to the build log.
fn write_log(log: &mut File, log_path: &Path, line: &str) -> anyhow::Result<()> {
    writeln!(log, "{line}").with_context(|| format!("cannot write `{}`", log_path.display()))
}

/// Print one line to stdout immediately, without buffering it.
fn stream(line: &str) {
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{line}").is_ok() {
        let _ = stdout.flush();
    }
}

/// The exit code and terminating signal of `status`, where available.
fn status_parts(status: ExitStatus) -> (Option<i32>, Option<i32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        (status.code(), status.signal())
    }
    #[cfg(not(unix))]
    {
        (status.code(), None)
    }
}

/// The bracketed exit status line that closes a step in the log.
fn format_exit_status(exit_code: Option<i32>, signal: Option<i32>) -> String {
    match (exit_code, signal) {
        (_, Some(signal)) => format!("[signal {signal}]"),
        (Some(code), None) => format!("[{code}]"),
        (None, None) => "[unknown]".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write an executable script at `path` with the given Unix mode.
    fn write_script(path: &Path, mode: u32) {
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_mode(mode);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        #[cfg(not(unix))]
        {
            let _ = mode;
        }
    }

    #[test]
    fn empty_program_is_not_resolved() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(resolve_program(root.path(), ""), None);
    }

    #[cfg(unix)]
    #[test]
    fn relative_path_is_taken_from_the_project_root() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("tools")).unwrap();
        write_script(&root.path().join("tools/make.sh"), 0o755);
        assert_eq!(
            resolve_program(root.path(), "tools/make.sh"),
            Some(root.path().join("tools/make.sh"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn absolute_path_is_used_as_is() {
        let root = tempfile::tempdir().unwrap();
        let tool = root.path().join("absolute.sh");
        write_script(&tool, 0o755);
        assert_eq!(
            resolve_program(Path::new("/nonexistent-root"), tool.to_str().unwrap()),
            Some(tool)
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_without_execute_bit_is_not_resolved() {
        let root = tempfile::tempdir().unwrap();
        write_script(&root.path().join("plain.sh"), 0o644);
        assert_eq!(resolve_program(root.path(), "plain.sh"), None);
    }

    #[cfg(unix)]
    #[test]
    fn directory_is_not_resolved() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("dir.sh")).unwrap();
        assert_eq!(resolve_program(root.path(), "dir.sh"), None);
    }

    #[cfg(unix)]
    #[test]
    fn name_is_looked_up_in_path() {
        assert!(resolve_program(Path::new("."), "sh").is_some());
    }
}
