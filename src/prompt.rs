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

//! Interactive questions asked by `texmake init` in a terminal.
//!
//! The real prompts come from [`inquire`]: a text prompt for the project name
//! and select prompts for the document type, the engine, the font and Chinese
//! support. [`collect`] keeps the question order and the defaults testable
//! without a terminal. Cancelling a question (Esc, Ctrl-C or end of input) is
//! reported as [`Error::Cancelled`], so `init` prints `已取消` and exits with
//! code 130 before writing anything.

use inquire::validator::Validation;
use inquire::{CustomUserError, InquireError, Select, Text};

use crate::engine::Engine;
use crate::error::Error;
use crate::init::{DocType, Font, ProjectType, font_options};

/// Labels of the questions.
const PROJECT_TYPE_PROMPT: &str = "项目类型";
const NAME_PROMPT: &str = "项目名";
const DOC_TYPE_PROMPT: &str = "文档类型";
const ENGINE_PROMPT: &str = "引擎";
const FONT_PROMPT: &str = "字体";
const CTEX_PROMPT: &str = "中文支持";

/// Display labels of the Chinese support choices.
const CTEX_YES: &str = "是";
const CTEX_NO: &str = "否";

/// The Chinese support choices, in display order.
fn ctex_options() -> [&'static str; 2] {
    [CTEX_YES, CTEX_NO]
}

/// The engine choices, in display order.
fn engine_choices() -> &'static [Engine] {
    const ENGINES: [Engine; 3] = [Engine::Pdflatex, Engine::Xelatex, Engine::Lualatex];
    &ENGINES
}

/// The starting cursor of a select prompt listing `options`: the index of
/// `default`, or the first choice when `options` does not contain it.
fn cursor<T: PartialEq>(options: &[T], default: T) -> usize {
    options
        .iter()
        .position(|option| *option == default)
        .unwrap_or(0)
}

/// The starting cursor of the project type prompt for `default`.
fn project_type_cursor(default: ProjectType) -> usize {
    cursor(&ProjectType::OPTIONS, default)
}

/// The starting cursor of the document type prompt for `default`.
fn doc_type_cursor(default: DocType) -> usize {
    cursor(&DocType::OPTIONS, default)
}

/// The starting cursor of the engine prompt for `default`.
fn engine_cursor(default: Engine) -> usize {
    cursor(engine_choices(), default)
}

/// The starting cursor of the font prompt for `default`.
///
/// A font that is not offered for `engine` falls back to the first choice.
fn font_cursor(engine: Engine, default: Font) -> usize {
    cursor(font_options(engine), default)
}

/// The starting cursor of the Chinese support prompt for `default`.
fn ctex_cursor(default: bool) -> usize {
    usize::from(!default)
}

/// The answers collected by `init` in a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answers {
    pub name: String,
    pub doc_type: DocType,
    pub engine: Engine,
    pub font: Font,
    pub ctex: bool,
}

/// Asks the questions of `init`; implemented by [`InquirePrompter`] in
/// production and by a fake in the tests.
pub trait Prompter {
    fn ask_project_type(&mut self, default: ProjectType) -> anyhow::Result<ProjectType>;
    fn ask_name(&mut self, default: &str) -> anyhow::Result<String>;
    fn ask_doc_type(&mut self, default: DocType) -> anyhow::Result<DocType>;
    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine>;
    fn ask_font(&mut self, engine: Engine, default: Font) -> anyhow::Result<Font>;
    fn ask_ctex(&mut self, default: bool) -> anyhow::Result<bool>;
}

/// Asks the questions with [`inquire`] prompts on the terminal.
pub struct InquirePrompter;

impl Prompter for InquirePrompter {
    fn ask_project_type(&mut self, default: ProjectType) -> anyhow::Result<ProjectType> {
        Select::new(PROJECT_TYPE_PROMPT, ProjectType::OPTIONS.to_vec())
            .with_starting_cursor(project_type_cursor(default))
            .prompt()
            .map_err(cancelled)
    }

    fn ask_name(&mut self, default: &str) -> anyhow::Result<String> {
        // `with_default` leaves the field empty and returns the default on an
        // empty submit, so typing replaces it; `with_initial_value` would
        // pre-fill the field and make typed text append to it.
        Text::new(NAME_PROMPT)
            .with_default(default)
            .with_validator(name_validator)
            .prompt()
            .map(|name| name.trim().to_owned())
            .map_err(cancelled)
    }

    fn ask_doc_type(&mut self, default: DocType) -> anyhow::Result<DocType> {
        Select::new(DOC_TYPE_PROMPT, DocType::OPTIONS.to_vec())
            .with_starting_cursor(doc_type_cursor(default))
            .prompt()
            .map_err(cancelled)
    }

    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine> {
        Select::new(ENGINE_PROMPT, engine_choices().to_vec())
            .with_starting_cursor(engine_cursor(default))
            .prompt()
            .map_err(cancelled)
    }

    fn ask_font(&mut self, engine: Engine, default: Font) -> anyhow::Result<Font> {
        Select::new(FONT_PROMPT, font_options(engine).to_vec())
            .with_starting_cursor(font_cursor(engine, default))
            .prompt()
            .map_err(cancelled)
    }

    fn ask_ctex(&mut self, default: bool) -> anyhow::Result<bool> {
        let choice = Select::new(CTEX_PROMPT, ctex_options().to_vec())
            .with_starting_cursor(ctex_cursor(default))
            .prompt()
            .map_err(cancelled)?;
        Ok(choice == CTEX_YES)
    }
}

/// Ask the document type, the engine, the font and Chinese support, in that
/// order; the project name is already confirmed and lives in `defaults`.
///
/// A cancelled question aborts the whole sequence.
pub fn collect(prompter: &mut dyn Prompter, defaults: Answers) -> anyhow::Result<Answers> {
    let doc_type = prompter.ask_doc_type(defaults.doc_type)?;
    let engine = prompter.ask_engine(defaults.engine)?;
    let font = prompter.ask_font(engine, defaults.font)?;
    let ctex = prompter.ask_ctex(defaults.ctex)?;
    Ok(Answers {
        name: defaults.name,
        doc_type,
        engine,
        font,
        ctex,
    })
}

/// Check a project name and return it without surrounding whitespace.
///
/// The name becomes a directory name, so `.`, `..` and path separators are
/// rejected as well as empty names.
pub fn validate_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("项目名不能为空".to_owned());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("项目名不能是 `.` 或 `..`".to_owned());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("项目名不能包含路径分隔符".to_owned());
    }
    Ok(trimmed.to_owned())
}

/// The [`inquire`] validator around [`validate_name`].
fn name_validator(value: &str) -> Result<Validation, CustomUserError> {
    validate_name(value)
        .map(|_| Validation::Valid)
        .map_err(Into::into)
}

/// Map a cancelled or failed prompt to an error the caller can exit on.
///
/// Esc and Ctrl-C are reported as cancellations by [`inquire`]; a closed input
/// arrives as [`std::io::ErrorKind::UnexpectedEof`]. Both produce the plain
/// [`Error::Cancelled`], which prints as `已取消` and exits with code 130
/// before anything is written.
fn cancelled(error: InquireError) -> anyhow::Error {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            Error::Cancelled.into()
        }
        InquireError::IO(ref io) if io.kind() == std::io::ErrorKind::UnexpectedEof => {
            Error::Cancelled.into()
        }
        other => anyhow::anyhow!("cannot ask the init questions: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakePrompter;

    fn defaults() -> Answers {
        Answers {
            name: "demo".to_owned(),
            doc_type: DocType::Article,
            engine: Engine::Xelatex,
            font: Font::NewComputerModern,
            ctex: true,
        }
    }

    #[test]
    fn collect_asks_the_document_type_then_engine_then_font_then_ctex() {
        let mut prompter = FakePrompter::new();
        let answers = collect(&mut prompter, defaults()).expect("answers");
        assert_eq!(prompter.calls, ["doc_type", "engine", "font", "ctex"]);
        assert_eq!(answers.name, "demo");
        assert_eq!(answers.doc_type, DocType::Book);
        assert_eq!(answers.engine, Engine::Pdflatex);
        assert_eq!(answers.font, Font::LatinModern);
        assert!(!answers.ctex);
    }

    #[test]
    fn collect_passes_the_defaults_to_each_question() {
        let mut prompter = FakePrompter::new();
        collect(&mut prompter, defaults()).expect("answers");
        assert_eq!(prompter.doc_type_default, Some(DocType::Article));
        assert_eq!(prompter.engine_default, Some(Engine::Xelatex));
        // The font prompt follows the chosen engine, not the default one.
        assert_eq!(prompter.font_engine, Some(Engine::Pdflatex));
        assert_eq!(prompter.font_default, Some(Font::NewComputerModern));
        assert_eq!(prompter.ctex_default, Some(true));
    }

    #[test]
    fn collect_stops_after_a_cancelled_document_type() {
        let mut prompter = FakePrompter::new();
        prompter.fail_at = Some("doc_type");
        let error = collect(&mut prompter, defaults()).expect_err("cancelled");
        assert_eq!(prompter.calls, ["doc_type"]);
        assert!(error.downcast_ref::<Error>().is_some());
    }

    #[test]
    fn collect_stops_after_a_cancelled_font() {
        let mut prompter = FakePrompter::new();
        prompter.fail_at = Some("font");
        collect(&mut prompter, defaults()).expect_err("cancelled");
        assert_eq!(prompter.calls, ["doc_type", "engine", "font"]);
    }

    #[test]
    fn validate_name_accepts_a_normal_name_and_trims_it() {
        assert_eq!(validate_name("thesis").as_deref(), Ok("thesis"));
        assert_eq!(validate_name("  thesis  ").as_deref(), Ok("thesis"));
    }

    #[test]
    fn validate_name_rejects_blank_names() {
        for blank in ["", "   ", "\t\n"] {
            assert_eq!(validate_name(blank), Err("项目名不能为空".to_owned()));
        }
    }

    #[test]
    fn validate_name_rejects_dot_and_dot_dot() {
        for name in [".", "..", " . "] {
            assert_eq!(
                validate_name(name),
                Err("项目名不能是 `.` 或 `..`".to_owned())
            );
        }
    }

    #[test]
    fn validate_name_rejects_path_separators() {
        for name in ["a/b", "a\\b", "/tmp/x"] {
            assert_eq!(
                validate_name(name),
                Err("项目名不能包含路径分隔符".to_owned())
            );
        }
    }

    #[test]
    fn the_project_type_choices_are_project_then_workspace() {
        assert_eq!(
            ProjectType::OPTIONS.map(|project_type| project_type.to_string()),
            ["项目", "工作区"]
        );
        assert_eq!(project_type_cursor(ProjectType::Project), 0);
        assert_eq!(project_type_cursor(ProjectType::Workspace), 1);
    }

    #[test]
    fn the_document_type_choices_are_article_then_book() {
        assert_eq!(
            DocType::OPTIONS.map(|doc_type| doc_type.to_string()),
            ["article", "book"]
        );
        assert_eq!(doc_type_cursor(DocType::Article), 0);
        assert_eq!(doc_type_cursor(DocType::Book), 1);
    }

    #[test]
    fn the_engine_choices_are_pdflatex_xelatex_lualatex() {
        assert_eq!(
            engine_choices()
                .iter()
                .map(Engine::to_string)
                .collect::<Vec<_>>(),
            ["pdflatex", "xelatex", "lualatex"]
        );
        assert_eq!(engine_cursor(Engine::Pdflatex), 0);
        assert_eq!(engine_cursor(Engine::Xelatex), 1);
        assert_eq!(engine_cursor(Engine::Lualatex), 2);
    }

    #[test]
    fn the_ctex_choices_are_yes_then_no() {
        assert_eq!(ctex_options(), ["是", "否"]);
        assert_eq!(ctex_cursor(true), 0);
        assert_eq!(ctex_cursor(false), 1);
    }

    #[test]
    fn the_font_cursor_falls_back_to_the_first_choice() {
        assert_eq!(font_cursor(Engine::Pdflatex, Font::LatinModern), 1);
        assert_eq!(font_cursor(Engine::Xelatex, Font::NewComputerModern), 0);
        assert_eq!(font_cursor(Engine::Pdflatex, Font::Fira), 0);
    }

    fn texmake_error(error: anyhow::Error) -> Error {
        error.downcast::<Error>().expect("a texmake error")
    }

    /// Assert that `error` is the plain cancellation error: no `error: ` prefix,
    /// the message `已取消`, and exit code 130.
    fn assert_cancelled(error: Error) {
        assert!(matches!(&error, Error::Cancelled), "got {error:?}");
        assert_eq!(error.exit_code(), 130);
        assert_eq!(error.to_string(), "已取消");
    }

    #[test]
    fn cancelled_maps_escape_to_cancelled() {
        assert_cancelled(texmake_error(cancelled(InquireError::OperationCanceled)));
    }

    #[test]
    fn cancelled_maps_ctrl_c_to_cancelled() {
        assert_cancelled(texmake_error(cancelled(InquireError::OperationInterrupted)));
    }

    #[test]
    fn cancelled_maps_end_of_input_to_cancelled() {
        let eof = InquireError::IO(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "end of input",
        ));
        assert_cancelled(texmake_error(cancelled(eof)));
    }

    #[test]
    fn the_prompt_labels_name_the_questions() {
        assert_eq!(NAME_PROMPT, "项目名");
        assert_eq!(DOC_TYPE_PROMPT, "文档类型");
        assert_eq!(ENGINE_PROMPT, "引擎");
        assert_eq!(FONT_PROMPT, "字体");
        assert_eq!(CTEX_PROMPT, "中文支持");
    }
}
