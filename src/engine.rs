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
