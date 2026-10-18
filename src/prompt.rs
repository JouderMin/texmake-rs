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
//! Prompts go to a writer (stdout in the real command) and answers come from a
//! buffered reader (stdin). Answers are validated with a caller-supplied
//! function; a rejected answer prints one error line and asks again. End of
//! input cancels the command with a usage error, so the caller exits with code
//! 2 before writing anything.

use std::io::{BufRead, Write};

use anyhow::Context;

use crate::error::usage;

/// Ask `question` until `validate` accepts the answer.
///
/// An empty answer selects `default`; without a default it is validated like
/// any other answer, so a validator that rejects empty input asks again.
/// Answers are trimmed. The prompt shows the default in brackets, for example
/// `项目名 [demo]: `.
pub fn ask(
    question: &str,
    default: Option<&str>,
    validate: impl Fn(&str) -> Result<String, String>,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> anyhow::Result<String> {
    loop {
        match default {
            Some(default) => write!(out, "{question} [{default}]: ")?,
            None => write!(out, "{question}: ")?,
        }
        out.flush()?;

        let mut line = String::new();
        let read = input
            .read_line(&mut line)
            .context("cannot read the answer from stdin")?;
        if read == 0 {
            return Err(usage(format!("init cancelled: no answer for `{question}`")));
        }

        let answer = line.trim();
        let candidate = if answer.is_empty() {
            default.unwrap_or_default()
        } else {
            answer
        };
        match validate(candidate) {
            Ok(value) => return Ok(value),
            Err(message) => writeln!(out, "{message}")?,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::error::Error;

    fn ask_with(
        answers: &str,
        default: Option<&str>,
        validate: impl Fn(&str) -> Result<String, String>,
    ) -> (Result<String, String>, String) {
        let mut input = Cursor::new(answers.as_bytes());
        let mut out = Vec::new();
        let result = ask("项目名", default, validate, &mut input, &mut out)
            .map_err(|error| error.to_string());
        let out = String::from_utf8(out).expect("stdout is UTF-8");
        (result, out)
    }

    fn non_empty(value: &str) -> Result<String, String> {
        let value = value.trim();
        if value.is_empty() {
            Err("the project name must not be empty".to_owned())
        } else {
            Ok(value.to_owned())
        }
    }

    #[test]
    fn enter_accepts_the_default() {
        let (answer, out) = ask_with("\n", Some("demo"), non_empty);
        assert_eq!(answer.expect("answer"), "demo");
        assert_eq!(out, "项目名 [demo]: ");
    }

    #[test]
    fn invalid_answers_are_asked_again() {
        let (answer, out) = ask_with("   \nok\n", None, non_empty);
        assert_eq!(answer.expect("answer"), "ok");
        assert_eq!(out, "项目名: the project name must not be empty\n项目名: ");
    }

    #[test]
    fn end_of_input_cancels_with_a_usage_error() {
        let mut input = Cursor::new(Vec::new());
        let mut out = Vec::new();
        let error = ask("项目名", Some("demo"), non_empty, &mut input, &mut out)
            .expect_err("end of input must cancel");
        let error = error
            .downcast_ref::<Error>()
            .expect("the cancellation is a texmake error");
        assert!(matches!(error, Error::Usage(message) if message.contains("cancelled")));
        assert_eq!(
            String::from_utf8(out).expect("stdout is UTF-8"),
            "项目名 [demo]: "
        );
    }

    #[test]
    fn empty_input_without_a_default_is_asked_again() {
        let (answer, out) = ask_with("\n\nreal\n", None, non_empty);
        assert_eq!(answer.expect("answer"), "real");
        assert_eq!(out.matches("项目名: ").count(), 3);
    }

    #[test]
    fn an_invalid_default_is_reported_and_the_question_repeats() {
        let validate = |value: &str| match value {
            "lualatex" => Ok(value.to_owned()),
            _ => Err("use `lualatex`".to_owned()),
        };
        let (answer, out) = ask_with("\n\nlualatex\n", Some("pdflatex"), validate);
        assert_eq!(answer.expect("answer"), "lualatex");
        assert_eq!(out.matches("use `lualatex`\n").count(), 2);
    }
}
