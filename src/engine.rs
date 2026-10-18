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

//! The LaTeX engines supported by `latex` steps and `texmake init`.
//!
//! One definition is the single source of truth for the engine names: the
//! executable to run, the spelling accepted in `texmake.toml`, the `--engine`
//! values and the generated configuration all derive from this enum.

use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;

/// A LaTeX engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Engine {
    Pdflatex,
    Xelatex,
    Lualatex,
}

impl Engine {
    /// The executable name of the engine.
    pub fn command(self) -> &'static str {
        match self {
            Engine::Pdflatex => "pdflatex",
            Engine::Xelatex => "xelatex",
            Engine::Lualatex => "lualatex",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.command())
    }
}

impl FromStr for Engine {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "pdflatex" => Ok(Engine::Pdflatex),
            "xelatex" => Ok(Engine::Xelatex),
            "lualatex" => Ok(Engine::Lualatex),
            other => Err(format!(
                "unknown latex engine `{other}`; \
                 expected `pdflatex`, `xelatex` or `lualatex`"
            )),
        }
    }
}
