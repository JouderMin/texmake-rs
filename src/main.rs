//! texmake entry point.
//!
//! `main` only delegates to [`texmake::cli::run`]; everything else lives in the
//! library crate.

fn main() {
    std::process::exit(texmake::cli::run());
}
