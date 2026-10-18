#![allow(clippy::unwrap_used)]

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

/// `PATH` entries with the real toolchains first and the fixture's `bin`
/// afterwards, then the system directories.
fn tool_path_entries(tool_directories: &[PathBuf], fixture_bin: &Path) -> Vec<PathBuf> {
    let mut directories = tool_directories.to_vec();
    directories.push(fixture_bin.to_path_buf());
    directories.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    directories
}

/// `PATH` with the real toolchains first and the fixture's `bin` afterwards.
///
/// [`Fixture::cmd`] only exposes `bin`, `/usr/bin` and `/bin`; a real build
/// needs the TeX installation and Graphviz on `PATH` as well. The installed
/// toolchains come first so a fixture-local fake tool (`xelatex`, `dot`) never
/// shadows the real program in the real-build tests.
fn real_tools_path(f: &Fixture) -> OsString {
    std::env::join_paths(tool_path_entries(&tool_directories(), &f.path("bin"))).unwrap()
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

/// A `texmake` command whose `PATH` holds only one empty directory.
///
/// No platform can resolve `xelatex`, `dot` or any other program through it,
/// so a test that expects a missing tool does not depend on the host's
/// `/usr/bin`, `/usr/local/bin` or Homebrew layout. The returned temporary
/// directory must stay alive for as long as the command runs.
fn cmd_without_tools(f: &Fixture) -> (assert_cmd::Command, tempfile::TempDir) {
    let empty = tempfile::tempdir().unwrap();
    let mut command = f.cmd();
    command.env("PATH", empty.path());
    (command, empty)
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
    f.write(
        "main.tex",
        "\\documentclass{article}\n\\begin{document}\nDraft.\n\\end{document}\n",
    );
    // Make the edited source newer than the PDF without relying on timestamp
    // granularity or a fixed sleep.
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    let edited = std::fs::OpenOptions::new()
        .write(true)
        .open(f.path("main.tex"))
        .unwrap();
    if let Err(error) = edited.set_modified(future) {
        eprintln!("skip: cannot set the modification time: {error}");
        return;
    }
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
    // `biblatex` with `backend=biber` writes `\entry{knuth}` records rather than
    // `\bibitem`, so the cited key and the resolved name are checked instead.
    let bbl = std::fs::read_to_string(f.path("build/main.bbl")).unwrap();
    assert!(bbl.contains("\\entry{knuth}"), "{bbl}");
    assert!(bbl.contains("family={Knuth}"), "{bbl}");
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

    // A project that already has `texmake.toml` is refused, dry run included.
    let configured = tempfile::tempdir().unwrap();
    std::fs::write(configured.path().join("texmake.toml"), "invalid\n").unwrap();
    let out = cmd_in(configured.path())
        .args(["init", "-n"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("refusing to overwrite"), "{err}");
    assert!(!configured.path().join("main.tex").exists());
    assert!(!configured.path().join(".gitignore").exists());
}

#[test]
fn missing_xelatex_reports_tool() {
    // `PATH` holds only an empty directory, so no platform can find xelatex.
    let f = Fixture::new();
    let (mut cmd, _empty) = cmd_without_tools(&f);
    let out = cmd.output().unwrap();
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
    let (mut dry, _empty) = cmd_without_tools(&f);
    let dry = dry.arg("-n").output().unwrap();
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
    let (mut build, _empty) = cmd_without_tools(&f);
    let build = build.output().unwrap();
    assert_eq!(
        build.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(String::from_utf8_lossy(&build.stderr).contains("dot"));
}

#[test]
fn fixture_dot_is_used_when_the_path_has_no_real_tools() {
    let f = Fixture::new();
    f.fake_tool("dot", "#!/bin/sh\nprintf 'fake dot\\n' > fake-dot.out\n");
    f.write_toml(
        "[project]\nname = \"fixture\"\nmain = \"main.tex\"\n\n\
         [tools.graphviz]\ncommand = \"dot\"\n\n\
         [recipes.release]\nsteps = [{ tool = \"graphviz\", args = [\"-Tpdf\", \"-o\", \"graph.pdf\"] }]\n",
    );
    // Only the fixture `bin` is visible: the fake `dot` must be picked up, and
    // a real Graphviz installation can never shadow it.
    let mut cmd = f.cmd();
    cmd.env("PATH", f.path("bin"));
    let out = cmd.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(f.path("fake-dot.out")).unwrap_or_default(),
        "fake dot\n"
    );
    let log = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    assert!(log.contains("$ dot -Tpdf -o graph.pdf"), "{log}");
}

#[test]
fn real_tools_path_prefers_installed_toolchains_over_the_fixture() {
    let f = Fixture::new();
    let fixture_bin = f.path("bin");
    // The ordering is asserted on synthetic tool directories first, so this
    // test also guards the invariant on machines without TeX or Graphviz.
    let synthetic = [
        PathBuf::from("/synthetic/tex"),
        PathBuf::from("/synthetic/graphviz"),
    ];
    let directories = tool_path_entries(&synthetic, &fixture_bin);
    let bin_index = directories
        .iter()
        .position(|directory| *directory == fixture_bin)
        .expect("the fixture bin must be on PATH");
    assert!(
        bin_index > 0,
        "the fixture bin must not come first: {:?}",
        directories
    );
    assert_eq!(directories.len(), synthetic.len() + 3);
    for tool_directory in &synthetic {
        let index = directories
            .iter()
            .position(|directory| directory == tool_directory)
            .expect("every tool directory must be on PATH");
        assert!(
            index < bin_index,
            "{} must precede the fixture bin",
            tool_directory.display()
        );
    }

    // The real path must place whatever toolchains exist before `bin` as well.
    let installed = tool_directories();
    if installed.is_empty() {
        eprintln!("skip: no installed toolchains on this machine");
        return;
    }
    let real: Vec<PathBuf> = std::env::split_paths(&real_tools_path(&f)).collect();
    let real_bin = real
        .iter()
        .position(|directory| *directory == fixture_bin)
        .expect("the fixture bin must be on PATH");
    assert_eq!(real_bin, installed.len(), "{:?}", real);
    for tool_directory in installed {
        let index = real
            .iter()
            .position(|directory| *directory == tool_directory)
            .expect("every tool directory must be on PATH");
        assert!(
            index < real_bin,
            "{} must precede the fixture bin",
            tool_directory.display()
        );
    }
}

#[test]
fn installed_xelatex_is_used_instead_of_a_fixture_shadow() {
    if !have("xelatex") {
        eprintln!("skip: xelatex not found");
        return;
    }
    let f = Fixture::new();
    // If the fixture bin shadowed the real toolchain, this script would run and
    // fail the build.
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nprintf 'shadow\\n' > shadow-xelatex.out\nexit 1\n",
    );
    let out = real_cmd(&f).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !f.path("shadow-xelatex.out").exists(),
        "the fixture xelatex must not shadow the installed one"
    );
    assert!(f.path("build/main.pdf").exists());
}
