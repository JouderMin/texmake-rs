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
//! and a select prompt for the language and the engine. [`collect`] keeps the
//! order (project name, language, engine) and the defaults testable without a
//! terminal. Cancelling a question (Esc, Ctrl-C or end of input) is reported as
//! [`Error::Cancelled`], so `init` prints `已取消` and exits with code 130
//! before writing anything.

use anyhow::anyhow;
use inquire::validator::Validation;
use inquire::{CustomUserError, InquireError, Select, Text};

use crate::engine::Engine;
use crate::error::Error;
use crate::init::Language;

/// Labels of the three questions.
const NAME_PROMPT: &str = "项目名";
const LANGUAGE_PROMPT: &str = "语言";
const ENGINE_PROMPT: &str = "引擎";

/// Display labels of the language choices, in the order shown by the prompt.
const LANGUAGE_ZH: &str = "zh 中文";
const LANGUAGE_EN: &str = "en English";

/// Display labels of the engine choices, in the order shown by the prompt.
const ENGINE_XELATEX: &str = "xelatex";
const ENGINE_LUALATEX: &str = "lualatex";

/// The language choices, in display order.
fn language_options() -> [&'static str; 2] {
    [LANGUAGE_ZH, LANGUAGE_EN]
}

/// The engine choices, in display order.
fn engine_options() -> [&'static str; 2] {
    [ENGINE_XELATEX, ENGINE_LUALATEX]
}

/// The starting cursor of the language prompt for `default`.
fn language_cursor(default: Language) -> usize {
    match default {
        Language::Zh => 0,
        Language::En => 1,
    }
}

/// The starting cursor of the engine prompt for `default`.
///
/// `pdflatex` cannot reach the prompt (`init` rejects it first); it maps to the
/// first supported choice instead of panicking.
fn engine_cursor(default: Engine) -> usize {
    usize::from(default == Engine::Lualatex)
}

/// The answers collected by `init` in a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answers {
    pub name: String,
    pub language: Language,
    pub engine: Engine,
}

/// Asks the questions of `init`; implemented by [`InquirePrompter`] in
/// production and by a fake in the tests.
pub trait Prompter {
    fn ask_name(&mut self, default: &str) -> anyhow::Result<String>;
    fn ask_language(&mut self, default: Language) -> anyhow::Result<Language>;
    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine>;
}

/// Asks the questions with [`inquire`] prompts on the terminal.
pub struct InquirePrompter;

impl Prompter for InquirePrompter {
    fn ask_name(&mut self, default: &str) -> anyhow::Result<String> {
        Text::new(NAME_PROMPT)
            .with_initial_value(default)
            .with_validator(validate_name)
            .prompt()
            .map(|name| name.trim().to_owned())
            .map_err(cancelled)
    }

    fn ask_language(&mut self, default: Language) -> anyhow::Result<Language> {
        let choice = Select::new(LANGUAGE_PROMPT, language_options().to_vec())
            .with_starting_cursor(language_cursor(default))
            .prompt()
            .map_err(cancelled)?;
        Ok(if choice == LANGUAGE_ZH {
            Language::Zh
        } else {
            Language::En
        })
    }

    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine> {
        let choice = Select::new(ENGINE_PROMPT, engine_options().to_vec())
            .with_starting_cursor(engine_cursor(default))
            .prompt()
            .map_err(cancelled)?;
        Ok(if choice == ENGINE_LUALATEX {
            Engine::Lualatex
        } else {
            Engine::Xelatex
        })
    }
}

/// Ask the project name, then the language, then the engine.
///
/// `defaults` holds the command-line values (or the directory name); the
/// prompts start with them. A cancelled question aborts the whole sequence.
pub fn collect(prompter: &mut dyn Prompter, defaults: Answers) -> anyhow::Result<Answers> {
    let name = prompter.ask_name(&defaults.name)?;
    let language = prompter.ask_language(defaults.language)?;
    let engine = prompter.ask_engine(defaults.engine)?;
    Ok(Answers {
        name,
        language,
        engine,
    })
}

/// Reject a project name that is empty or only whitespace.
fn validate_name(value: &str) -> Result<Validation, CustomUserError> {
    if value.trim().is_empty() {
        return Err("项目名不能为空".into());
    }
    Ok(Validation::Valid)
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
        other => anyhow!("cannot ask the init questions: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::usage;

    #[derive(Debug)]
    struct FakePrompter {
        calls: Vec<&'static str>,
        name_default: Option<String>,
        language_default: Option<Language>,
        engine_default: Option<Engine>,
        fail_at: Option<&'static str>,
        name: String,
        language: Language,
        engine: Engine,
    }

    impl FakePrompter {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                name_default: None,
                language_default: None,
                engine_default: None,
                fail_at: None,
                name: "thesis".to_owned(),
                language: Language::En,
                engine: Engine::Lualatex,
            }
        }
    }

    impl Prompter for FakePrompter {
        fn ask_name(&mut self, default: &str) -> anyhow::Result<String> {
            self.calls.push("name");
            self.name_default = Some(default.to_owned());
            if self.fail_at == Some("name") {
                return Err(usage("init cancelled: test".to_owned()));
            }
            Ok(self.name.clone())
        }

        fn ask_language(&mut self, default: Language) -> anyhow::Result<Language> {
            self.calls.push("language");
            self.language_default = Some(default);
            if self.fail_at == Some("language") {
                return Err(usage("init cancelled: test".to_owned()));
            }
            Ok(self.language)
        }

        fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine> {
            self.calls.push("engine");
            self.engine_default = Some(default);
            if self.fail_at == Some("engine") {
                return Err(usage("init cancelled: test".to_owned()));
            }
            Ok(self.engine)
        }
    }

    fn defaults() -> Answers {
        Answers {
            name: "demo".to_owned(),
            language: Language::Zh,
            engine: Engine::Xelatex,
        }
    }

    #[test]
    fn collect_asks_the_name_then_the_language_then_the_engine() {
        let mut prompter = FakePrompter::new();
        let answers = collect(&mut prompter, defaults()).expect("answers");
        assert_eq!(prompter.calls, ["name", "language", "engine"]);
        assert_eq!(answers.name, "thesis");
        assert_eq!(answers.language, Language::En);
        assert_eq!(answers.engine, Engine::Lualatex);
    }

    #[test]
    fn collect_passes_the_defaults_to_each_question() {
        let mut prompter = FakePrompter::new();
        collect(&mut prompter, defaults()).expect("answers");
        assert_eq!(prompter.name_default.as_deref(), Some("demo"));
        assert_eq!(prompter.language_default, Some(Language::Zh));
        assert_eq!(prompter.engine_default, Some(Engine::Xelatex));
    }

    #[test]
    fn collect_reports_a_cancelled_question_as_a_usage_error() {
        let mut prompter = FakePrompter::new();
        prompter.fail_at = Some("engine");
        let error = collect(&mut prompter, defaults()).expect_err("cancelled");
        let error = error
            .downcast_ref::<Error>()
            .expect("the cancellation is a texmake error");
        assert!(matches!(&error, Error::Usage(message) if message.contains("cancelled")));
    }

    #[test]
    fn the_name_validator_accepts_a_normal_name() {
        assert!(matches!(validate_name("thesis"), Ok(Validation::Valid)));
        assert!(matches!(validate_name("  thesis  "), Ok(Validation::Valid)));
    }

    #[test]
    fn the_name_validator_rejects_blank_names_with_a_message() {
        for blank in ["", "   ", "\t\n"] {
            let error = match validate_name(blank) {
                Err(error) => error,
                Ok(_) => panic!("`{blank:?}` must be rejected"),
            };
            assert_eq!(error.to_string(), "项目名不能为空");
        }
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
    fn collect_stops_after_a_cancelled_name() {
        let mut prompter = FakePrompter::new();
        prompter.fail_at = Some("name");
        let error = collect(&mut prompter, defaults()).expect_err("cancelled");
        assert_eq!(prompter.calls, ["name"]);
        assert!(
            matches!(texmake_error(error), Error::Usage(message) if message.contains("cancelled"))
        );
    }

    #[test]
    fn collect_stops_after_a_cancelled_language() {
        let mut prompter = FakePrompter::new();
        prompter.fail_at = Some("language");
        let error = collect(&mut prompter, defaults()).expect_err("cancelled");
        assert_eq!(prompter.calls, ["name", "language"]);
        assert!(
            matches!(texmake_error(error), Error::Usage(message) if message.contains("cancelled"))
        );
    }

    #[test]
    fn the_language_choices_are_zh_then_en() {
        assert_eq!(language_options(), ["zh 中文", "en English"]);
    }

    #[test]
    fn the_engine_choices_are_xelatex_then_lualatex() {
        assert_eq!(engine_options(), ["xelatex", "lualatex"]);
    }

    #[test]
    fn the_starting_cursors_point_at_the_defaults() {
        assert_eq!(language_cursor(Language::Zh), 0);
        assert_eq!(language_cursor(Language::En), 1);
        assert_eq!(engine_cursor(Engine::Xelatex), 0);
        assert_eq!(engine_cursor(Engine::Lualatex), 1);
    }

    #[test]
    fn the_prompt_labels_name_the_questions() {
        assert_eq!(NAME_PROMPT, "项目名");
        assert_eq!(LANGUAGE_PROMPT, "语言");
        assert_eq!(ENGINE_PROMPT, "引擎");
    }
}
