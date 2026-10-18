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
//! a usage error, so `init` exits with code 2 before writing anything.

use anyhow::anyhow;
use inquire::validator::Validation;
use inquire::{CustomUserError, InquireError, Select, Text};

use crate::engine::Engine;
use crate::error::usage;
use crate::init::Language;

/// Display labels of the language choices, in the order shown by the prompt.
const LANGUAGE_ZH: &str = "zh 中文";
const LANGUAGE_EN: &str = "en English";

/// Display labels of the engine choices, in the order shown by the prompt.
const ENGINE_XELATEX: &str = "xelatex";
const ENGINE_LUALATEX: &str = "lualatex";

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
        Text::new("项目名")
            .with_initial_value(default)
            .with_validator(validate_name)
            .prompt()
            .map(|name| name.trim().to_owned())
            .map_err(cancelled)
    }

    fn ask_language(&mut self, default: Language) -> anyhow::Result<Language> {
        let cursor = match default {
            Language::Zh => 0,
            Language::En => 1,
        };
        let choice = Select::new("语言", vec![LANGUAGE_ZH, LANGUAGE_EN])
            .with_starting_cursor(cursor)
            .prompt()
            .map_err(cancelled)?;
        Ok(if choice == LANGUAGE_ZH {
            Language::Zh
        } else {
            Language::En
        })
    }

    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine> {
        let cursor = usize::from(default == Engine::Lualatex);
        let choice = Select::new("引擎", vec![ENGINE_XELATEX, ENGINE_LUALATEX])
            .with_starting_cursor(cursor)
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
/// arrives as [`std::io::ErrorKind::UnexpectedEof`]. Both end the command with a
/// usage error before anything is written.
fn cancelled(error: InquireError) -> anyhow::Error {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            usage("init cancelled: the questions were interrupted".to_owned())
        }
        InquireError::IO(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            usage("init cancelled: the input ended".to_owned())
        }
        other => anyhow!("cannot ask the init questions: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

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
        assert!(matches!(error, Error::Usage(message) if message.contains("cancelled")));
    }

    #[test]
    fn the_name_validator_rejects_blank_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(matches!(validate_name("thesis"), Ok(Validation::Valid)));
    }
}
