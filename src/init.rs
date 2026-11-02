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
//! `init` is interactive: it asks whether to create a project or a workspace
//! root, then the project name and, for a project, the document type, the
//! engine, the font and Chinese support. The files are written into a directory
//! named after the answer. The name is confirmed before the directory is
//! created; cancelling any later question removes that (still empty) directory
//! again. A workspace gets only `texmake.toml` and `.gitignore`; its member
//! projects are added later by hand. With `dry_run` the questions are still
//! asked and the planned files are printed instead of written.

use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Context;

use crate::config::FILE_NAME;
use crate::engine::Engine;
use crate::error::usage;
use crate::prompt;

/// What `init` creates: one project, or a workspace root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    /// A single document project.
    Project,
    /// A workspace root that lists member projects.
    Workspace,
}

impl ProjectType {
    /// The project type choices, in display order.
    pub const OPTIONS: [ProjectType; 2] = [ProjectType::Project, ProjectType::Workspace];
}

impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ProjectType::Project => "项目",
            ProjectType::Workspace => "工作区",
        })
    }
}

/// Document class of the generated `main.tex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Article,
    Book,
}

impl DocType {
    /// The document type choices, in display order.
    pub const OPTIONS: [DocType; 2] = [DocType::Article, DocType::Book];
}

impl fmt::Display for DocType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DocType::Article => "article",
            DocType::Book => "book",
        })
    }
}

/// Font choice of the generated `main.tex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    ComputerModern,
    LatinModern,
    NewComputerModern,
    NewComputerModernSans,
    Concrete,
    Euler,
    Fira,
    StixTwo,
}

impl Font {
    /// Display label shown by the font prompt.
    fn label(self) -> &'static str {
        match self {
            Font::ComputerModern => "Computer Modern",
            Font::LatinModern => "Latin Modern",
            Font::NewComputerModern => "New Computer Modern",
            Font::NewComputerModernSans => "New Computer Modern Sans",
            Font::Concrete => "Concrete",
            Font::Euler => "Euler",
            Font::Fira => "Fira",
            Font::StixTwo => "STIX Two",
        }
    }

    /// The `fontsetup` option for a Unicode engine, or `None` for `pdflatex`
    /// fonts.
    fn fontsetup_option(self) -> Option<&'static str> {
        match self {
            Font::ComputerModern | Font::LatinModern => None,
            Font::NewComputerModern => Some("default"),
            Font::NewComputerModernSans => Some("sansdefault"),
            Font::Concrete => Some("concrete"),
            Font::Euler => Some("euler"),
            Font::Fira => Some("fira"),
            Font::StixTwo => Some("stixtwo"),
        }
    }

    /// The preamble line for a `pdflatex` font.
    fn pdflatex_preamble(self) -> Option<&'static str> {
        match self {
            Font::ComputerModern => Some("\\usepackage[T1]{fontenc}"),
            Font::LatinModern => Some("\\usepackage{lmodern}"),
            _ => None,
        }
    }

    /// Whether `ctex` can be combined with this font.
    fn supports_chinese(self) -> bool {
        self != Font::ComputerModern
    }
}

impl fmt::Display for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The font choices offered for `engine`, in display order.
pub fn font_options(engine: Engine) -> &'static [Font] {
    const PDFLATEX: [Font; 2] = [Font::ComputerModern, Font::LatinModern];
    const UNICODE: [Font; 6] = [
        Font::NewComputerModern,
        Font::NewComputerModernSans,
        Font::Concrete,
        Font::Euler,
        Font::Fira,
        Font::StixTwo,
    ];
    match engine {
        Engine::Pdflatex => &PDFLATEX,
        Engine::Xelatex | Engine::Lualatex => &UNICODE,
    }
}

/// Everything `texmake init` needs from the command line.
#[derive(Debug)]
pub struct InitOptions {
    /// Ask the questions but only report what would be created.
    pub dry_run: bool,
    /// Suppress progress lines (`created`, `would ...`).
    pub quiet: bool,
}

/// Create a new document project in the current directory.
///
/// `init` needs a terminal: without one the questions cannot be asked, so the
/// command fails before touching the file system.
pub fn run_init(opts: &InitOptions) -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        return Err(usage(
            "init needs a terminal: run it in an interactive shell".to_owned(),
        ));
    }
    let cwd = std::env::current_dir().context("cannot read working directory")?;
    let mut prompter = prompt::InquirePrompter;
    run_init_in(&mut prompter, opts, &cwd, &write_new_file)
}

/// Ask every question and create the project below `cwd`.
///
/// The project directory is created right after the name is confirmed, so a
/// cancelled later question or a failed write removes it again. `prompter` and
/// `writer` are parameters so the flow can be tested without a terminal or a
/// failing disk.
fn run_init_in(
    prompter: &mut dyn prompt::Prompter,
    opts: &InitOptions,
    cwd: &Path,
    writer: &dyn Fn(&Path, &str) -> io::Result<()>,
) -> anyhow::Result<()> {
    let project_type = prompter.ask_project_type(ProjectType::Project)?;
    let name = prompter.ask_name("")?;
    let name = prompt::validate_name(&name).map_err(usage)?;
    let dir = cwd.join(&name);

    let mut created = false;
    if !opts.dry_run {
        if dir.exists() {
            return Err(usage(format!("`{}` already exists", dir.display())));
        }
        std::fs::create_dir(&dir).with_context(|| format!("cannot create `{}`", dir.display()))?;
        created = true;
    }

    let defaults = prompt::Answers {
        name,
        doc_type: DocType::Article,
        engine: Engine::Xelatex,
        font: Font::NewComputerModern,
        ctex: false,
    };
    let outcome = match project_type {
        ProjectType::Project => create_project(prompter, opts, &dir, defaults, writer),
        ProjectType::Workspace => create_workspace(opts, &dir, writer),
    };
    if outcome.is_err() && created {
        // Every failure below leaves the new directory empty, so removing it
        // cannot delete user data.
        let _ = std::fs::remove_dir(&dir);
    }
    outcome
}

/// Ask for the document layout and write the files.
fn create_project(
    prompter: &mut dyn prompt::Prompter,
    opts: &InitOptions,
    dir: &Path,
    defaults: prompt::Answers,
    writer: &dyn Fn(&Path, &str) -> io::Result<()>,
) -> anyhow::Result<()> {
    let answers = prompt::collect(prompter, defaults)?;
    if answers.ctex && !answers.font.supports_chinese() {
        return Err(usage(
            "Computer Modern does not support Chinese: choose Latin Modern, or use xelatex or lualatex with a Unicode font"
                .to_owned(),
        ));
    }
    let files = planned_files(&answers);
    if opts.dry_run {
        print_would_create(&files, opts.quiet);
        return Ok(());
    }
    write_project(dir, &files, opts, writer)
}

/// Create a workspace root: a `[workspace]` configuration with no members yet,
/// and the same `.gitignore` a project gets. There is no `main.tex`.
fn create_workspace(
    opts: &InitOptions,
    dir: &Path,
    writer: &dyn Fn(&Path, &str) -> io::Result<()>,
) -> anyhow::Result<()> {
    let files = workspace_files();
    if opts.dry_run {
        print_would_create(&files, opts.quiet);
        return Ok(());
    }
    write_project(dir, &files, opts, writer)
}

/// Report the files a dry run would create, in reporting order.
fn print_would_create(files: &[(&'static str, String)], quiet: bool) {
    if quiet {
        return;
    }
    for (label, _) in files {
        println!("would create {label}");
    }
}

/// The files a workspace root is made of, in reporting order.
fn workspace_files() -> [(&'static str, String); 2] {
    [
        (FILE_NAME, "[workspace]\nmembers = []\n".to_owned()),
        (".gitignore", "/build/\n".to_owned()),
    ]
}

/// The files `init` creates, in reporting order.
fn planned_files(answers: &prompt::Answers) -> [(&'static str, String); 3] {
    [
        (FILE_NAME, config_template(&answers.name, answers.engine)),
        ("main.tex", main_template(answers)),
        (".gitignore", "/build/\n".to_owned()),
    ]
}

/// Write the planned files into `dir`.
///
/// `main.tex` and `.gitignore` are written before `texmake.toml`, so a failure
/// never leaves a configuration without its document files. Files created by
/// this call are removed again when a later write fails. `writer` is a
/// parameter so tests can inject a failing write.
fn write_project(
    dir: &Path,
    files: &[(&'static str, String)],
    opts: &InitOptions,
    writer: &dyn Fn(&Path, &str) -> io::Result<()>,
) -> anyhow::Result<()> {
    // Write the configuration last, so a failure never leaves a configuration
    // without the document files it describes.
    let (config, documents): (Vec<_>, Vec<_>) =
        files.iter().partition(|(label, _)| *label == FILE_NAME);
    let mut created = Vec::new();
    let outcome = (|| -> anyhow::Result<()> {
        for (label, content) in documents.iter().chain(config.iter()) {
            let path = dir.join(label);
            match writer(&path, content) {
                Ok(()) => {
                    created.push(path);
                    if !opts.quiet {
                        println!("created {label}");
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(usage(format!("refusing to overwrite `{}`", path.display())));
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("cannot create `{}`", path.display()));
                }
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
fn main_template(answers: &prompt::Answers) -> String {
    let font_line = match answers.font.fontsetup_option() {
        Some(option) => format!("\\usepackage[{option}]{{fontsetup}}"),
        None => answers
            .font
            .pdflatex_preamble()
            .expect("a font without a fontsetup option is a pdflatex font")
            .to_owned(),
    };
    let mut preamble = font_line;
    if answers.ctex {
        preamble.push('\n');
        preamble.push_str("\\usepackage[UTF8]{ctex}");
    }
    let (class, body) = match (answers.doc_type, answers.ctex) {
        (DocType::Article, false) => ("article", "Hello, world."),
        (DocType::Article, true) => ("article", "你好，世界。"),
        (DocType::Book, false) => ("book", "\\chapter{Introduction}\nHello, world."),
        (DocType::Book, true) => ("book", "\\chapter{引言}\n你好，世界。"),
    };
    format!(
        "\\documentclass{{{class}}}
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
    use crate::error::Error;
    use crate::prompt::Answers;

    fn answers(doc_type: DocType, engine: Engine, font: Font, ctex: bool) -> Answers {
        Answers {
            name: "thesis".to_owned(),
            doc_type,
            engine,
            font,
            ctex,
        }
    }

    #[test]
    fn pdflatex_offers_two_fonts() {
        assert_eq!(font_options(Engine::Pdflatex).len(), 2);
        assert_eq!(font_options(Engine::Pdflatex)[0], Font::ComputerModern);
        assert_eq!(font_options(Engine::Pdflatex)[1], Font::LatinModern);
    }

    #[test]
    fn unicode_engines_offer_six_fonts() {
        for engine in [Engine::Xelatex, Engine::Lualatex] {
            let fonts = font_options(engine);
            assert_eq!(fonts.len(), 6);
            assert_eq!(fonts[0], Font::NewComputerModern);
            assert_eq!(fonts[5], Font::StixTwo);
        }
    }

    #[test]
    fn the_template_matrix_has_the_expected_font_and_body_lines() {
        for doc_type in DocType::OPTIONS {
            for engine in [Engine::Pdflatex, Engine::Xelatex, Engine::Lualatex] {
                for &font in font_options(engine) {
                    for ctex in [false, true] {
                        if ctex && !font.supports_chinese() {
                            continue;
                        }
                        let answers = answers(doc_type, engine, font, ctex);
                        let main = main_template(&answers);
                        let class = doc_type.to_string();
                        assert!(main.contains(&format!("\\documentclass{{{class}}}")));
                        let font_line = match font.fontsetup_option() {
                            Some(option) => format!("\\usepackage[{option}]{{fontsetup}}"),
                            None => font.pdflatex_preamble().unwrap().to_owned(),
                        };
                        assert!(
                            main.contains(&font_line),
                            "{doc_type:?} {engine} {font} ctex={ctex}: {main}"
                        );
                        assert_eq!(main.contains("\\usepackage[UTF8]{ctex}"), ctex);
                        if ctex {
                            let font_at = main.find(&font_line).unwrap();
                            let ctex_at = main.find("\\usepackage[UTF8]{ctex}").unwrap();
                            assert!(
                                font_at < ctex_at,
                                "{doc_type:?} {engine} {font}: the font line must precede ctex: {main}"
                            );
                        }
                        match (doc_type, ctex) {
                            (DocType::Article, false) => {
                                assert!(main.contains("Hello, world."));
                            }
                            (DocType::Article, true) => {
                                assert!(main.contains("你好，世界。"));
                            }
                            (DocType::Book, false) => {
                                assert!(main.contains("\\chapter{Introduction}"));
                                assert!(main.contains("Hello, world."));
                            }
                            (DocType::Book, true) => {
                                assert!(main.contains("\\chapter{引言}"));
                                assert!(main.contains("你好，世界。"));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn computer_modern_with_chinese_is_rejected() {
        let mut prompter = FakePrompter::new(
            DocType::Article,
            Engine::Pdflatex,
            Font::ComputerModern,
            true,
        );
        let dir = tempfile::tempdir().unwrap();
        let error = run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file)
            .expect_err("rejected");
        let error = error.downcast::<Error>().expect("a usage error");
        assert!(error.to_string().contains("Computer Modern"), "{error}");
        assert!(!dir.path().join("thesis").exists());
    }

    #[test]
    fn a_cancelled_question_removes_the_created_directory() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        prompter.fail_at = Some("engine");
        let dir = tempfile::tempdir().unwrap();
        let error = run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file)
            .expect_err("cancelled");
        assert!(matches!(
            error.downcast_ref::<Error>(),
            Some(Error::Cancelled)
        ));
        assert!(!dir.path().join("thesis").exists());
    }

    #[test]
    fn a_write_failure_removes_the_created_directory() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        let dir = tempfile::tempdir().unwrap();
        let writer = |path: &Path, content: &str| {
            if path.ends_with(".gitignore") {
                return Err(io::Error::other("disk full"));
            }
            write_new_file(path, content)
        };
        let error =
            run_init_in(&mut prompter, &quiet(), dir.path(), &writer).expect_err("write fails");
        assert!(error.to_string().contains(".gitignore"), "{error}");
        assert!(
            !dir.path().join("thesis").exists(),
            "the created project directory must be removed"
        );
    }

    #[test]
    fn writes_the_project_described_by_the_answers() {
        let mut prompter = FakePrompter::new(DocType::Book, Engine::Xelatex, Font::Concrete, false);
        let dir = tempfile::tempdir().unwrap();
        run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file).expect("project");
        let project = dir.path().join("thesis");
        let config = std::fs::read_to_string(project.join("texmake.toml")).unwrap();
        assert!(config.contains("name = \"thesis\""));
        assert!(config.contains("{ latex = \"xelatex\" }"));
        let main = std::fs::read_to_string(project.join("main.tex")).unwrap();
        assert!(main.contains("\\documentclass{book}"));
        assert!(main.contains("\\usepackage[concrete]{fontsetup}"));
        assert!(main.contains("\\chapter{Introduction}"));
        assert!(!main.contains("ctex"));
        assert_eq!(
            std::fs::read_to_string(project.join(".gitignore")).unwrap(),
            "/build/\n"
        );
    }

    #[test]
    fn an_existing_directory_is_refused_before_asking_layout() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("thesis")).unwrap();
        let error =
            run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file).expect_err("exists");
        assert!(error.to_string().contains("already exists"), "{error}");
        assert_eq!(prompter.calls, ["type", "name"]);
    }

    #[test]
    fn a_workspace_gets_a_root_configuration_and_no_document() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        prompter.project_type = ProjectType::Workspace;
        let dir = tempfile::tempdir().unwrap();
        run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file).expect("workspace");
        let workspace = dir.path().join("thesis");
        assert_eq!(
            std::fs::read_to_string(workspace.join("texmake.toml")).unwrap(),
            "[workspace]\nmembers = []\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join(".gitignore")).unwrap(),
            "/build/\n"
        );
        assert!(!workspace.join("main.tex").exists());
        assert_eq!(prompter.calls, ["type", "name"]);
    }

    #[test]
    fn a_workspace_dry_run_writes_nothing() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        prompter.project_type = ProjectType::Workspace;
        let dir = tempfile::tempdir().unwrap();
        let opts = InitOptions {
            dry_run: true,
            quiet: true,
        };
        run_init_in(&mut prompter, &opts, dir.path(), &write_new_file).expect("dry run");
        assert!(!dir.path().join("thesis").exists());
    }

    #[test]
    fn cancelling_the_project_type_creates_nothing() {
        let mut prompter =
            FakePrompter::new(DocType::Article, Engine::Xelatex, Font::Concrete, false);
        prompter.project_type = ProjectType::Workspace;
        prompter.fail_at = Some("type");
        let dir = tempfile::tempdir().unwrap();
        let error =
            run_init_in(&mut prompter, &quiet(), dir.path(), &write_new_file).expect_err("cancel");
        assert!(matches!(
            error.downcast_ref::<Error>(),
            Some(Error::Cancelled)
        ));
        assert!(!dir.path().join("thesis").exists());
        assert_eq!(prompter.calls, ["type"]);
    }

    #[test]
    fn dry_run_prints_the_planned_files_and_writes_nothing() {
        let mut prompter = FakePrompter::new(
            DocType::Article,
            Engine::Lualatex,
            Font::NewComputerModern,
            true,
        );
        let dir = tempfile::tempdir().unwrap();
        let opts = InitOptions {
            dry_run: true,
            quiet: false,
        };
        run_init_in(&mut prompter, &opts, dir.path(), &write_new_file).expect("dry run");
        assert!(!dir.path().join("thesis").exists());
    }

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
    fn failed_config_write_rolls_back_created_files() {
        let dir = tempfile::tempdir().unwrap();
        let files = planned_files(&answers(
            DocType::Article,
            Engine::Xelatex,
            Font::Concrete,
            false,
        ));
        let writer = |path: &Path, content: &str| {
            if path.ends_with("texmake.toml") {
                return Err(io::Error::other("disk full"));
            }
            write_new_file(path, content)
        };
        let error = write_project(dir.path(), &files, &quiet(), &writer).unwrap_err();
        assert!(error.to_string().contains("texmake.toml"), "{error}");
        assert!(!dir.path().join("texmake.toml").exists());
        assert!(!dir.path().join("main.tex").exists());
        assert!(!dir.path().join(".gitignore").exists());
    }

    fn quiet() -> InitOptions {
        InitOptions {
            dry_run: false,
            quiet: true,
        }
    }

    struct FakePrompter {
        calls: Vec<&'static str>,
        fail_at: Option<&'static str>,
        project_type: ProjectType,
        doc_type: DocType,
        engine: Engine,
        font: Font,
        ctex: bool,
    }

    impl FakePrompter {
        fn new(doc_type: DocType, engine: Engine, font: Font, ctex: bool) -> Self {
            Self {
                calls: Vec::new(),
                fail_at: None,
                project_type: ProjectType::Project,
                doc_type,
                engine,
                font,
                ctex,
            }
        }
    }

    impl prompt::Prompter for FakePrompter {
        fn ask_project_type(&mut self, _default: ProjectType) -> anyhow::Result<ProjectType> {
            self.calls.push("type");
            self.fail_if("type")?;
            Ok(self.project_type)
        }

        fn ask_name(&mut self, _default: &str) -> anyhow::Result<String> {
            self.calls.push("name");
            Ok("thesis".to_owned())
        }

        fn ask_doc_type(&mut self, _default: DocType) -> anyhow::Result<DocType> {
            self.calls.push("doc_type");
            self.fail_if("doc_type")?;
            Ok(self.doc_type)
        }

        fn ask_engine(&mut self, _default: Engine) -> anyhow::Result<Engine> {
            self.calls.push("engine");
            self.fail_if("engine")?;
            Ok(self.engine)
        }

        fn ask_font(&mut self, _engine: Engine, _default: Font) -> anyhow::Result<Font> {
            self.calls.push("font");
            self.fail_if("font")?;
            Ok(self.font)
        }

        fn ask_ctex(&mut self, _default: bool) -> anyhow::Result<bool> {
            self.calls.push("ctex");
            self.fail_if("ctex")?;
            Ok(self.ctex)
        }
    }

    impl FakePrompter {
        fn fail_if(&self, step: &'static str) -> anyhow::Result<()> {
            if self.fail_at == Some(step) {
                return Err(Error::Cancelled.into());
            }
            Ok(())
        }
    }
}
