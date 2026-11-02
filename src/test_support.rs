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

//! Test doubles shared by the unit tests of several modules.

use crate::engine::Engine;
use crate::error::Error;
use crate::init::{DocType, Font, ProjectType};
use crate::prompt::Prompter;

/// A [`Prompter`] that records the questions it was asked, the defaults it was
/// given, and either returns canned answers or cancels at a chosen step.
#[derive(Debug)]
pub(crate) struct FakePrompter {
    pub calls: Vec<&'static str>,
    pub fail_at: Option<&'static str>,
    pub project_type: ProjectType,
    pub doc_type: DocType,
    pub engine: Engine,
    pub font: Font,
    pub ctex: bool,
    pub project_type_default: Option<ProjectType>,
    pub doc_type_default: Option<DocType>,
    pub engine_default: Option<Engine>,
    pub font_engine: Option<Engine>,
    pub font_default: Option<Font>,
    pub ctex_default: Option<bool>,
}

impl FakePrompter {
    /// A prompter that answers with the defaults of `init`.
    pub fn new() -> Self {
        Self::with_answers(DocType::Book, Engine::Pdflatex, Font::LatinModern, false)
    }

    /// A prompter that answers the document layout with the given values.
    pub fn with_answers(doc_type: DocType, engine: Engine, font: Font, ctex: bool) -> Self {
        Self {
            calls: Vec::new(),
            fail_at: None,
            project_type: ProjectType::Project,
            doc_type,
            engine,
            font,
            ctex,
            project_type_default: None,
            doc_type_default: None,
            engine_default: None,
            font_engine: None,
            font_default: None,
            ctex_default: None,
        }
    }

    /// Return `value` unless this is the step the fake was told to cancel at.
    fn answer<T>(&self, step: &'static str, value: T) -> anyhow::Result<T> {
        if self.fail_at == Some(step) {
            return Err(Error::Cancelled.into());
        }
        Ok(value)
    }
}

impl Default for FakePrompter {
    fn default() -> Self {
        Self::new()
    }
}

impl Prompter for FakePrompter {
    fn ask_project_type(&mut self, default: ProjectType) -> anyhow::Result<ProjectType> {
        self.calls.push("type");
        self.project_type_default = Some(default);
        self.answer("type", self.project_type)
    }

    fn ask_name(&mut self, _default: &str) -> anyhow::Result<String> {
        self.calls.push("name");
        Ok("thesis".to_owned())
    }

    fn ask_doc_type(&mut self, default: DocType) -> anyhow::Result<DocType> {
        self.calls.push("doc_type");
        self.doc_type_default = Some(default);
        self.answer("doc_type", self.doc_type)
    }

    fn ask_engine(&mut self, default: Engine) -> anyhow::Result<Engine> {
        self.calls.push("engine");
        self.engine_default = Some(default);
        self.answer("engine", self.engine)
    }

    fn ask_font(&mut self, engine: Engine, default: Font) -> anyhow::Result<Font> {
        self.calls.push("font");
        self.font_engine = Some(engine);
        self.font_default = Some(default);
        self.answer("font", self.font)
    }

    fn ask_ctex(&mut self, default: bool) -> anyhow::Result<bool> {
        self.calls.push("ctex");
        self.ctex_default = Some(default);
        self.answer("ctex", self.ctex)
    }
}
