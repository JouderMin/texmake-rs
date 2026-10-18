#![allow(clippy::unwrap_used)]
#![allow(dead_code)]

//! Shared integration-test fixture.
//!
//! Each test gets a temporary project directory seeded with a minimal
//! `texmake.toml` and `main.tex`. `cmd()` builds a `texmake` invocation rooted
//! there, with `PATH` pointing at the fixture-local `bin/` directory first so
//! tests can install fake tools.

use std::path::PathBuf;

const DEFAULT_CONFIG: &str = r#"[project]
name = "fixture"
main = "main.tex"

[recipes.release]
steps = [{ latex = "xelatex" }]
"#;

const DEFAULT_MAIN: &str = r#"\documentclass{article}
\begin{document}
Hello from the fixture.
\end{document}
"#;

/// A temporary LaTeX project used by integration tests.
pub struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    /// Create a fresh project with the default `texmake.toml` and `main.tex`.
    pub fn new() -> Self {
        let fixture = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        fixture.write("texmake.toml", DEFAULT_CONFIG);
        fixture.write("main.tex", DEFAULT_MAIN);
        std::fs::create_dir_all(fixture.path("bin")).unwrap();
        fixture
    }

    /// Absolute path of `rel` inside the project.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Write `content` to `rel`, creating parent directories as needed.
    pub fn write(&self, rel: &str, content: &str) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// Replace `texmake.toml` with `toml`.
    pub fn write_toml(&self, toml: &str) {
        self.write("texmake.toml", toml);
    }

    /// Install an executable fake tool named `name` in `bin/` and return `bin`.
    pub fn fake_tool(&self, name: &str, script: &str) -> PathBuf {
        let bin = self.path("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let tool = bin.join(name);
        std::fs::write(&tool, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&tool).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&tool, permissions).unwrap();
        }
        bin
    }

    /// A `texmake` command rooted at the project with a fixture-local `PATH`.
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut command = assert_cmd::Command::cargo_bin("texmake").unwrap();
        command.current_dir(self.dir.path()).env(
            "PATH",
            format!("{}:/usr/bin:/bin", self.path("bin").display()),
        );
        command
    }
}
