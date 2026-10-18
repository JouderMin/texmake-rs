//! Acceptance tests for the design document's `§12` examples.
//!
//! Most of `§12` is covered by the per-command integration tests in
//! `tests/cli.rs`; this file exercises the real TeX toolchain and the
//! remaining end-to-end flows. Tests that need an installed tool print a skip
//! message and pass when the tool is missing, so the suite stays runnable on a
//! machine without TeX Live.

mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use common::Fixture;

/// Whether `program` is found through the real `PATH`.
fn have(program: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {program} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Absolute path of `program` found through the real `PATH`.
fn which(program: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("sh")
        .args(["-c", &format!("command -v {program}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The directories holding the real toolchains, in `PATH` order.
fn tool_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for program in ["xelatex", "biber", "makeindex", "dot"] {
        if let Some(directory) =
            which(program).and_then(|path| path.parent().map(Path::to_path_buf))
            && !directories.contains(&directory)
        {
            directories.push(directory);
        }
    }
    directories
}

/// `PATH` with the fixture's `bin` first and the real toolchains afterwards.
///
/// [`Fixture::cmd`] only exposes `bin`, `/usr/bin` and `/bin`; a real build
/// needs the TeX installation and Graphviz on `PATH` as well.
fn real_tools_path(f: &Fixture) -> OsString {
    let mut directories = vec![f.path("bin")];
    directories.extend(tool_directories());
    directories.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    std::env::join_paths(directories).unwrap()
}

/// A `texmake` command rooted at the fixture, seeing the real toolchains.
fn real_cmd(f: &Fixture) -> assert_cmd::Command {
    let mut command = f.cmd();
    command.env("PATH", real_tools_path(f));
    command
}

/// A `texmake` command rooted at `directory`, seeing the real toolchains.
fn cmd_in(directory: &Path) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::cargo_bin("texmake").unwrap();
    command.current_dir(directory).env(
        "PATH",
        std::env::join_paths(
            tool_directories()
                .into_iter()
                .chain([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]),
        )
        .unwrap(),
    );
    command
}

/// The lines of the build log.
fn log_lines(f: &Fixture) -> Vec<String> {
    std::fs::read_to_string(f.path("build/texmake.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Two recipes with the same output directory, for the rebuild scenario.
const TWO_RECIPES: &str = r#"[project]
name = "fixture"
main = "main.tex"

[recipes.release]
steps = [{ latex = "xelatex" }]

[recipes.draft]
steps = [{ latex = "xelatex" }]
"#;

#[test]
fn real_build_produces_pdf() {
    if !have("xelatex") {
        eprintln!("skip: xelatex not found");
        return;
    }
    let f = Fixture::new();
    let out = real_cmd(&f).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(f.path("build/main.pdf").exists());
    let log = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    assert!(log.contains("$ xelatex"), "{log}");
}

#[test]
fn real_build_skips_when_up_to_date() {
    if !have("xelatex") {
        eprintln!("skip: xelatex not found");
        return;
    }
    let f = Fixture::new();
    assert_eq!(real_cmd(&f).output().unwrap().status.code(), Some(0));
    let before = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    let out = real_cmd(&f).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Everything is up to date."),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        std::fs::read_to_string(f.path("build/texmake.log")).unwrap(),
        before
    );
}

#[test]
fn real_draft_rebuild_runs_xelatex_once() {
    if !have("xelatex") {
        eprintln!("skip: xelatex not found");
        return;
    }
    let f = Fixture::new();
    f.write_toml(TWO_RECIPES);
    let first = real_cmd(&f).args(["build", "release"]).output().unwrap();
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write(
        "main.tex",
        "\\documentclass{article}\n\\begin{document}\nDraft.\n\\end{document}\n",
    );
    let out = real_cmd(&f).args(["build", "draft"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        log_lines(&f)
            .iter()
            .filter(|line| line.starts_with("$ xelatex"))
            .count(),
        1
    );
    assert!(f.path("build/main.pdf").exists());
}

#[test]
fn real_biber_bibliography() {
    if !have("xelatex") || !have("biber") {
        eprintln!("skip: toolchain not found");
        return;
    }
    let f = Fixture::new();
    f.write(
        "main.tex",
        "\\documentclass{article}\n\\usepackage[backend=biber]{biblatex}\n\
         \\addbibresource{refs.bib}\n\\begin{document}\nCiting \\cite{knuth}.\n\
         \\printbibliography\n\\end{document}\n",
    );
    f.write(
        "refs.bib",
        "@book{knuth,\n  author = {Donald Knuth},\n  title = {The Art of Computer Programming},\n\
         \x20 year = {1968},\n  publisher = {Addison-Wesley},\n}\n",
    );
    f.write_toml(
        "[project]\nname = \"fixture\"\nmain = \"main.tex\"\n[recipes.release]\n\
         steps = [{ latex = \"xelatex\" }, { biber = true }, { latex = \"xelatex\" }, { latex = \"xelatex\" }]\n",
    );
    let out = real_cmd(&f).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        f.path("build/main.bbl").exists(),
        "biber must write main.bbl"
    );
    let pdf = std::fs::metadata(f.path("build/main.pdf")).unwrap();
    assert!(pdf.len() > 0, "the bibliography build must produce a PDF");
}

#[test]
fn init_then_real_build() {
    if !have("xelatex") {
        eprintln!("skip: xelatex not found");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let init = cmd_in(directory.path()).arg("init").output().unwrap();
    assert_eq!(
        init.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(directory.path().join("texmake.toml").exists());
    assert!(directory.path().join("main.tex").exists());
    let build = cmd_in(directory.path()).output().unwrap();
    assert_eq!(
        build.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(directory.path().join("build/main.pdf").exists());
}

#[test]
fn init_acceptance_covers_dry_run_quiet_and_existing_files() {
    // `-n` reports what it would do and writes nothing.
    let fresh = tempfile::tempdir().unwrap();
    let dry = cmd_in(fresh.path()).args(["init", "-n"]).output().unwrap();
    assert_eq!(
        dry.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(stdout.contains("would create texmake.toml"), "{stdout}");
    assert!(stdout.contains("would create main.tex"), "{stdout}");
    assert!(stdout.contains("would create .gitignore"), "{stdout}");
    for file in ["texmake.toml", "main.tex", ".gitignore"] {
        assert!(
            !fresh.path().join(file).exists(),
            "{file} must not be written by -n"
        );
    }

    // An existing `main.tex` is kept, reported, and never overwritten.
    let existing = tempfile::tempdir().unwrap();
    std::fs::write(existing.path().join("main.tex"), "custom\n").unwrap();
    let dry = cmd_in(existing.path())
        .args(["init", "-n"])
        .output()
        .unwrap();
    assert_eq!(
        dry.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(stdout.contains("would skip existing main.tex"), "{stdout}");
    assert!(stdout.contains("would create texmake.toml"), "{stdout}");
    assert!(!existing.path().join("texmake.toml").exists());
    let init = cmd_in(existing.path()).arg("init").output().unwrap();
    assert_eq!(
        init.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let stdout = String::from_utf8_lossy(&init.stdout);
    assert!(stdout.contains("skipped existing main.tex"), "{stdout}");
    assert_eq!(
        std::fs::read_to_string(existing.path().join("main.tex")).unwrap(),
        "custom\n"
    );
    assert!(existing.path().join("texmake.toml").exists());
    assert!(existing.path().join(".gitignore").exists());

    // `-q` writes the same files without printing anything.
    let quiet = tempfile::tempdir().unwrap();
    let out = cmd_in(quiet.path()).args(["init", "-q"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    for file in ["texmake.toml", "main.tex", ".gitignore"] {
        assert!(quiet.path().join(file).exists(), "{file} must be created");
    }

    // A non-regular `main.tex` is rejected before anything is written.
    let broken = tempfile::tempdir().unwrap();
    std::fs::create_dir(broken.path().join("main.tex")).unwrap();
    let out = cmd_in(broken.path()).arg("init").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("main.tex"), "{err}");
    assert!(!broken.path().join("texmake.toml").exists());
    assert!(!broken.path().join(".gitignore").exists());
}

#[test]
fn missing_xelatex_reports_tool() {
    // The fixture `PATH` hides any installed TeX installation.
    let f = Fixture::new();
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("xelatex"), "{err}");
}

#[test]
fn configured_tool_dry_run_and_missing_tool() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"fixture\"\nmain = \"main.tex\"\n\n\
         [tools.graphviz]\ncommand = \"dot\"\n\n\
         [recipes.release]\nsteps = [{ tool = \"graphviz\", args = [\"-Tpdf\", \"-o\", \"graph.pdf\"] }]\n",
    );
    let dry = f.cmd().arg("-n").output().unwrap();
    assert_eq!(
        dry.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(
        String::from_utf8_lossy(&dry.stdout).contains("dot -Tpdf -o graph.pdf"),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    let build = f.cmd().output().unwrap();
    assert_eq!(
        build.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(String::from_utf8_lossy(&build.stderr).contains("dot"));
}
