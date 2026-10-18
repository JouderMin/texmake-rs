# texmake

A make-like build tool for LaTeX projects: declare the build order in `texmake.toml`, and texmake runs the steps in that order and stops at the first failure.

## Installation

```sh
cargo install --path .
# or build a binary in target/release/texmake
cargo build --release
```

## Quick start

```sh
texmake init thesis   # create texmake.toml, main.tex and .gitignore
cd thesis
texmake               # build the default recipe into build/main.pdf
texmake check         # validate the configuration
texmake clean         # remove every recipe's output directory
```

`texmake init [dir]` accepts `--name <name>`, `--engine <pdflatex|xelatex|lualatex>` (default `xelatex`) and `--language <zh|en>` (default `zh`). It refuses to overwrite an existing `texmake.toml`, keeps other existing files, and rejects `--engine pdflatex --language zh`.

## Configuration

texmake searches upward from the current directory for the nearest `texmake.toml`; `--file <path>` selects another file. The directory containing the configuration is the project root, and every relative path is resolved against it.

```toml
[project]
name = "thesis"              # required
main = "main.tex"            # required
output = "build"             # optional, defaults to "build"
default = "release"          # optional

[tools.graphviz]
command = "dot"
args = ["-Tpdf", "-o", "{output}/graph.pdf"]

[recipes.release]
steps = [
  { latex = "xelatex" },
  { biber = true },
  { latex = "xelatex" },
  { latex = "xelatex" },
]

[recipes.draft]
main = "draft.tex"           # optional override
output = "build/draft"       # optional override
steps = [{ latex = "xelatex" }]
```

Unknown keys, missing required keys, wrong types, and duplicate keys are configuration errors. Every error reports the file, line, column, key and reason.

## Commands

| Command | Description |
|---|---|
| `texmake` | Build the default recipe. |
| `texmake build [name]` | Build the default or the named recipe. |
| `texmake init [dir]` | Create a new document project. |
| `texmake clean` | Remove every recipe's output directory. |
| `texmake check` | Validate the configuration and print the project and recipes. |

With no `default` configured, a single recipe is used; with several recipes texmake asks for a name. An unknown recipe name lists all available recipes.

## Options

| Option | Effect |
|---|---|
| `-f, --file <path>` | Use the given configuration file. |
| `-B, --force` | Ignore the up-to-date check. |
| `-n, --dry-run` | Print every command without executing it; also skips the up-to-date check and the tool check. |
| `-v, --verbose` | Print full commands and stream tool output. |
| `-q, --quiet` | Print errors only. |

## Steps

| Key | Example | Behavior |
|---|---|---|
| `latex` | `{ latex = "xelatex" }` | Runs `pdflatex`, `xelatex` or `lualatex` in the project root with `-interaction=nonstopmode -halt-on-error -file-line-error -output-directory=<output>`, then the step `args` and the main document. |
| `biber` | `{ biber = true }` | Runs `biber` in the project root with the step `args` and `<output>/<main stem>`. |
| `bibtex` | `{ bibtex = true }` | Runs `bibtex` the same way as `biber`. |
| `makeindex` | `{ makeindex = true }` | Runs `makeindex` in the output directory with the step `args` and the main stem. |
| `run` | `{ run = "sh scripts/gen.sh" }` | Runs the whole command through the system shell in the project root. `args` is rejected. |
| `tool` | `{ tool = "graphviz", args = ["{root}/graph.dot"] }` | Runs a custom tool directly, without a shell. |

Each step has exactly one action key. `biber`, `bibtex` and `makeindex` must be `true`; `latex` accepts only `pdflatex`, `xelatex` and `lualatex`. Built-in steps accept extra arguments through `args`, which are appended after texmake's own arguments.

## Variables

These variables are available in `run` commands, step `args`, and custom tool `command`/`args`:

| Variable | Value |
|---|---|
| `{root}` | Absolute path of the project root. |
| `{main}` | Main document path, relative to the project root. |
| `{main_stem}` | Main document file name without its extension. |
| `{output}` | Output directory path, relative to the project root. |
| `{name}` | Project name. |
| `{recipe}` | Current recipe name. |

Substitution is single-pass, so substituted values are not rescanned. `{{` and `}}` produce literal braces. Any other `{identifier}` must be one of the six variables, otherwise it is a configuration error. Other brace contents such as `awk '{print $1}'` are kept verbatim. `${...}` is reserved for the shell and is only allowed in `run` steps; non-`run` steps also reject `~`. Non-`run` steps never go through a shell, so `$VAR` is passed through literally, and quotes do not protect variables.

## Custom tools

`[tools]` defines executables for `tool` steps:

```toml
[tools.graphviz]
command = "dot"                       # required
args = ["-Tpdf", "-o", "{output}/graph.pdf"]

[tools]
pandoc = "pandoc"                     # shorthand for command only
```

Step `args` are appended after the tool definition's `args`. A command containing a path separator is resolved relative to the project root; otherwise it is looked up on `PATH`. Tools run in the project root. Use `run` when you need pipes, redirections or other shell syntax.

## Up-to-date check

A recipe is skipped (with a message) when all of these hold: it contains a `latex` step, the main PDF exists, and the PDF is strictly newer than every regular file under the project root. The scan does not follow symbolic links and ignores `.git` and every recipe's output directory, and the loaded configuration file's modification time is compared too. `--force` rebuilds unconditionally. Recipes without a `latex` step always run. A skipped build does not create the output directory and does not check tools.

## Logs, failures and exit codes

Each build truncates `<output>/texmake.log` and records every rendered command, its merged stdout/stderr in arrival order, and its exit status. On failure texmake prints the failing command, the exit code, the last 20 log lines and the log path. `-v` streams tool output while it runs; `-q` still prints the failure report.

| Exit code | Meaning |
|---|---|
| 0 | Success. |
| 1 | Build failure, including a missing tool. |
| 2 | Configuration, usage or environment error. |

Execution stops at the first failing step. Missing tools are reported together before execution starts, and the check covers `latex`, `biber`, `bibtex`, `makeindex` and `tool` steps, but not `run` steps.

## 0.1 limitations

- The main document must be a `.tex` file directly in the project root.
- Only whole-recipe incremental builds exist; there is no step-level incremental build.
- There is no watch mode, and no shared common steps between recipes.
- The main PDF is named after the main document's stem.
- macOS and Linux are supported; Windows is best-effort.
