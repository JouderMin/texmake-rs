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

//! Template variable substitution for recipe step arguments.
//!
//! Steps may use the six variables [`Vars`] carries (`{root}`, `{main}`,
//! `{main_stem}`, `{output}`, `{name}`, `{recipe}`). [`render`] performs one
//! single-pass scan over the template:
//!
//! - `{{` and `}}` produce the literal braces `{` and `}`.
//! - `{identifier}` that is not one of the six variables is an error, so typos
//!   do not silently reach LaTeX.
//! - Any other brace group is copied verbatim, which keeps shell snippets such
//!   as `awk '{print $1}'` intact.
//! - `${...}` is shell syntax: it is copied verbatim in `run` steps and
//!   rejected everywhere else, and `~` is likewise rejected outside `run`.
//! - Substituted values are not rescanned, so variables cannot expand into
//!   other variables.

/// The values available to [`render`].
pub struct Vars<'a> {
    /// Absolute path of the project root.
    pub root: &'a str,
    /// Main document path, relative to the project root.
    pub main: &'a str,
    /// File name of the main document without its extension.
    pub main_stem: &'a str,
    /// Output directory, relative to the project root.
    pub output: &'a str,
    /// Project name.
    pub name: &'a str,
    /// Name of the recipe being built.
    pub recipe: &'a str,
}

impl Vars<'_> {
    /// Look up one of the six variable names.
    fn lookup(&self, name: &str) -> Option<&str> {
        match name {
            "root" => Some(self.root),
            "main" => Some(self.main),
            "main_stem" => Some(self.main_stem),
            "output" => Some(self.output),
            "name" => Some(self.name),
            "recipe" => Some(self.recipe),
            _ => None,
        }
    }
}

/// Expand the variables in `template`.
///
/// `is_run` marks templates that become shell commands: only those may use
/// `${...}` and `~`. Errors describe the offending construct; turning them into
/// configuration errors with a file position is the caller's job.
pub fn render(template: &str, vars: &Vars<'_>, is_run: bool) -> Result<String, String> {
    // Only a shell expands `~`, and `run` steps are the only templates that
    // reach a shell. Checking up front also catches `~` inside literal braces.
    if !is_run && template.contains('~') {
        return Err("`~` is not expanded outside run steps".to_owned());
    }

    // Same reasoning for `${...}`: checking the whole template up front also
    // catches it inside a literal brace group such as `{a${HOME}}`.
    if !is_run && template.contains("${") {
        return Err("shell expansion `${...}` is only allowed in run steps".to_owned());
    }

    let bytes = template.as_bytes();
    let mut rendered = String::with_capacity(template.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                // `{{` is an escaped literal `{`.
                if bytes.get(index + 1) == Some(&b'{') {
                    rendered.push('{');
                    index += 2;
                    continue;
                }

                // A complete `{identifier}` is a variable reference.
                if let Some((name, close)) = identifier_at(bytes, index + 1)
                    && bytes.get(close) == Some(&b'}')
                {
                    match vars.lookup(name) {
                        Some(value) => rendered.push_str(value),
                        None => return Err(format!("unknown variable `{name}`")),
                    }
                    index = close + 1;
                    continue;
                }

                // Everything else is copied verbatim up to the next `}`; an
                // unterminated group is copied to the end.
                match template[index..].find('}') {
                    Some(offset) => {
                        let end = index + offset + 1;
                        rendered.push_str(&template[index..end]);
                        index = end;
                    }
                    None => {
                        rendered.push_str(&template[index..]);
                        break;
                    }
                }
            }
            b'}' if bytes.get(index + 1) == Some(&b'}') => {
                rendered.push('}');
                index += 2;
            }
            b'}' => {
                rendered.push('}');
                index += 1;
            }
            b'$' if bytes.get(index + 1) == Some(&b'{') => {
                if !is_run {
                    return Err("shell expansion `${...}` is only allowed in run steps".to_owned());
                }
                let end = shell_expansion_end(bytes, index + 1);
                rendered.push_str(&template[index..end]);
                index = end;
            }
            b'$' => {
                rendered.push('$');
                index += 1;
            }
            _ => {
                // Copy the whole run of plain text at once instead of one
                // character at a time.
                let end = next_special(bytes, index);
                rendered.push_str(&template[index..end]);
                index = end;
            }
        }
    }

    Ok(rendered)
}

/// The first index at or after `start` that may start a substitution.
///
/// Returns `bytes.len()` when the rest of the template is plain text.
fn next_special(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'}' | b'$' => return index,
            _ => index += 1,
        }
    }
    index
}

/// Match `[A-Za-z_][A-Za-z0-9_]*` starting at `start`.
///
/// Returns the identifier and the index just past it.
fn identifier_at(bytes: &[u8], start: usize) -> Option<(&str, usize)> {
    let first = *bytes.get(start)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    let mut end = start + 1;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    // The identifier is ASCII, so this slice is valid UTF-8.
    let identifier = std::str::from_utf8(&bytes[start..end]).expect("identifier is ASCII");
    Some((identifier, end))
}

/// Find the end of a `${...}` shell expansion whose `{` is at `open`.
///
/// Nested braces are counted so `${VAR:-${OTHER}}` stays intact. An
/// unterminated expansion extends to the end of the template.
fn shell_expansion_end(bytes: &[u8], open: usize) -> usize {
    debug_assert_eq!(bytes.get(open), Some(&b'{'));
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(root: &'a str, recipe: &'a str) -> Vars<'a> {
        Vars {
            root,
            main: "main.tex",
            main_stem: "main",
            output: "build",
            name: "thesis",
            recipe,
        }
    }

    #[test]
    fn replaces_six_variables() {
        let v = vars("/p", "release");
        assert_eq!(
            render("{output}/{main_stem}.pdf", &v, false).unwrap(),
            "build/main.pdf"
        );
        assert_eq!(
            render("{recipe}:{name}:{main}:{root}", &v, false).unwrap(),
            "release:thesis:main.tex:/p"
        );
    }

    #[test]
    fn double_braces_are_literal() {
        assert_eq!(
            render("{{name}}", &vars("/p", "r"), false).unwrap(),
            "{name}"
        );
    }

    #[test]
    fn unknown_variable_is_error() {
        let err = render("{foo}", &vars("/p", "r"), false).unwrap_err();
        assert!(err.contains("unknown variable `foo`"), "{err}");
    }

    #[test]
    fn awk_braces_pass_through() {
        assert_eq!(
            render("awk '{print $1}'", &vars("/p", "r"), false).unwrap(),
            "awk '{print $1}'"
        );
    }

    #[test]
    fn shell_variable_only_in_run_steps() {
        assert_eq!(
            render("echo ${HOME}", &vars("/p", "r"), true).unwrap(),
            "echo ${HOME}"
        );
        let err = render("echo ${HOME}", &vars("/p", "r"), false).unwrap_err();
        assert!(err.contains("run"), "{err}");
    }

    #[test]
    fn tilde_rejected_outside_run() {
        assert!(render("~/x", &vars("/p", "r"), false).is_err());
    }

    #[test]
    fn substitution_is_single_pass() {
        let v = Vars {
            root: "/a{name}b",
            main: "main.tex",
            main_stem: "main",
            output: "build",
            name: "thesis",
            recipe: "r",
        };
        assert_eq!(render("{root}", &v, false).unwrap(), "/a{name}b");
    }

    #[test]
    fn unmatched_brace_is_literal() {
        assert_eq!(
            render("{unclosed", &vars("/p", "r"), false).unwrap(),
            "{unclosed"
        );
    }

    #[test]
    fn quotes_do_not_protect_variables() {
        assert_eq!(
            render("'{name}'", &vars("/p", "r"), false).unwrap(),
            "'thesis'"
        );
    }

    #[test]
    fn plain_dollar_variable_passes_through() {
        assert_eq!(
            render("$HOME/x", &vars("/p", "r"), false).unwrap(),
            "$HOME/x"
        );
    }

    #[test]
    fn tilde_inside_literal_braces_is_still_rejected() {
        assert!(render("{a~b}", &vars("/p", "r"), false).is_err());
    }

    #[test]
    fn shell_expansion_inside_literal_braces_is_rejected() {
        let err = render("{a${HOME}}", &vars("/p", "r"), false).unwrap_err();
        assert!(err.contains("run"), "{err}");
    }
}
