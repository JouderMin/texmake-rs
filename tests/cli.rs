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

#![allow(clippy::unwrap_used)]

mod common;

use common::{
    Fixture, cmd_at, fake_counting_tool, fake_pdf_tool, run_count, write_member, write_workspace,
};

#[test]
fn help_lists_all_commands() {
    let out = assert_cmd::Command::cargo_bin("texmake")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for cmd in ["build", "init", "clean", "check"] {
        assert!(text.contains(cmd), "missing {cmd} in: {text}");
    }
}

#[test]
fn unknown_subcommand_exits_2() {
    let out = assert_cmd::Command::cargo_bin("texmake")
        .unwrap()
        .arg("bogus")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn version_prints_name() {
    let out = assert_cmd::Command::cargo_bin("texmake")
        .unwrap()
        .arg("--version")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("texmake"));
}

#[test]
fn missing_config_exits_2() {
    let f = Fixture::new();
    std::fs::remove_file(f.path("texmake.toml")).unwrap();
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no texmake.toml found"));
}

#[test]
fn parent_directory_config_is_not_discovered() {
    let f = Fixture::new();
    std::fs::create_dir(f.path("sub")).unwrap();
    let out = f.cmd().current_dir(f.path("sub")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no texmake.toml found"));
}

#[test]
fn explicit_missing_config_exits_2() {
    let f = Fixture::new();
    let out = f.cmd().args(["--file", "nope.toml"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_key_reports_line() {
    let f = Fixture::new();
    f.write_toml("[project]\nname = \"thesis\"\nmain = \"main.tex\"\nbogus = 1\n");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("line 4"), "{err}");
    assert!(err.contains("unknown field `bogus`"), "{err}");
}

#[test]
fn type_error_reports_line() {
    let f = Fixture::new();
    f.write_toml("[project]\nname = 1\nmain = \"main.tex\"\n");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("line 2"));
}

/// Run `check` against `toml`, expect exit code 2 and `keyword` in stderr.
fn check_config_error(toml: &str, keyword: &str) {
    let f = Fixture::new();
    f.write_toml(toml);
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains(keyword), "missing {keyword:?} in: {stderr}");
}

#[test]
fn check_prints_project_and_recipes() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"thesis\"\nmain = \"main.tex\"\ndefault = \"release\"\n\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n\n\
         [recipes.draft]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("project: thesis"), "{text}");
    assert!(text.contains("release (default)"), "{text}");
    assert!(text.contains("draft"), "{text}");
}

#[test]
fn output_dot_is_config_error() {
    let f = Fixture::new();
    f.write_toml("[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \".\"\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n");
    let out = f.cmd().arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("output"));
}

#[test]
fn main_missing_on_disk_is_config_error() {
    let f = Fixture::new();
    std::fs::remove_file(f.path("main.tex")).unwrap();
    let out = f.cmd().arg("check").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("main"));
}

#[test]
fn explicit_file_sets_project_root() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("cfg")).unwrap();
    std::fs::copy(f.path("main.tex"), f.path("cfg/main.tex")).unwrap();
    std::fs::write(
        f.path("cfg/texmake.toml"),
        "[project]\nname = \"x\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    )
    .unwrap();
    let out = f
        .cmd()
        .args(["--file", "cfg/texmake.toml", "check"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("project: x"));
}

#[test]
fn undefined_tool_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ tool = \"graphviz\" }]\n",
        "unknown tool",
    );
}

#[test]
fn undefined_default_recipe_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\ndefault = \"release\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
        "default",
    );
}

#[test]
fn main_in_subdirectory_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"src/main.tex\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
        "main",
    );
}

#[test]
fn main_not_tex_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.md\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
        "main",
    );
}

#[test]
fn empty_steps_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = []\n",
        "steps",
    );
}

#[test]
fn no_recipes_is_config_error() {
    check_config_error("[project]\nname = \"t\"\nmain = \"main.tex\"\n", "recipe");
}

#[test]
fn step_with_two_actions_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ latex = \"xelatex\", biber = true }]\n",
        "action",
    );
}

#[test]
fn false_biber_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ biber = false }]\n",
        "biber",
    );
}

#[test]
fn unknown_latex_engine_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ latex = \"tex\" }]\n",
        "xelatex",
    );
}

#[test]
fn run_with_args_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[recipes.a]\nsteps = [{ run = \"x\", args = [] }]\n",
        "args",
    );
}

#[test]
fn tool_table_without_command_is_config_error() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[tools.pandoc]\nargs = [\"-o\", \"x\"]\n\n[recipes.a]\nsteps = [{ tool = \"pandoc\" }]\n",
        "command",
    );
}

#[test]
fn tool_table_unknown_key_reports_line() {
    check_config_error(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n[tools.pandoc]\nbogus = 1\n",
        "line 5",
    );
}

#[test]
fn tool_string_shorthand_is_valid() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[tools]\npandoc = \"pandoc\"\n\n[recipes.a]\nsteps = [{ tool = \"pandoc\" }]\n",
    );
    assert_eq!(
        f.cmd().arg("check").output().unwrap().status.code(),
        Some(0)
    );
}

#[test]
fn tool_table_with_command_and_args_is_valid() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n[tools.graphviz]\ncommand = \"dot\"\nargs = [\"-Tpdf\", \"-o\", \"{output}/graph.pdf\"]\n\n[recipes.a]\nsteps = [{ tool = \"graphviz\" }, { latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dry_run_prints_resolved_commands_in_order() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n\
         [tools.graphviz]\ncommand = \"dot\"\nargs = [\"-Tpdf\", \"-o\", \"{output}/graph.pdf\"]\n\n\
         [recipes.release]\nsteps = [\n\
         { latex = \"xelatex\", args = [\"-shell-escape\"] },\n\
         { biber = true },\n\
         { bibtex = true },\n\
         { makeindex = true },\n\
         { tool = \"graphviz\", args = [\"graph.dot\"] },\n\
         { run = \"sh scripts/gen.sh\" },\n]\n",
    );
    let out = f.cmd().args(["-n", "build", "release"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec![
            "xelatex -interaction=nonstopmode -halt-on-error -file-line-error -output-directory=build -shell-escape main.tex",
            "biber build/main",
            "bibtex build/main",
            "makeindex main",
            "dot -Tpdf -o build/graph.pdf graph.dot",
            "sh scripts/gen.sh",
        ]
    );
}

#[test]
fn dry_run_creates_no_output_dir_and_skips_tool_check() {
    let f = Fixture::new();
    let out = f.cmd().args(["-n"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!f.path("build").exists());
}

#[test]
fn unknown_recipe_lists_available_names() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n\
         [recipes.draft]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().args(["build", "nope"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("unknown") && err.contains("release") && err.contains("draft"),
        "{err}"
    );
}

#[test]
fn multiple_recipes_without_default_is_error() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n\
         [recipes.draft]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().args(["-n"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unreferenced_tool_templates_are_validated() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n\
         [tools.graphviz]\ncommand = \"dot\"\nargs = [\"{bogus}\"]\n\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains("unknown variable"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn symlinked_output_ancestor_outside_root_is_config_error() {
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), f.path("build")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"build/out\"\n\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains("outside"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn symlinked_ancestor_with_dotdot_output_is_config_error() {
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), f.path("build")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"build/../escaped\"\n\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains("outside"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn dangling_symlink_output_is_config_error() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.path("missing-target"), f.path("build")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"build/thesis\"\n\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "stderr: {stderr}");
    assert!(stderr.contains("symbolic link"), "{stderr}");
}

#[test]
fn missing_output_directory_without_symlinks_is_allowed() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"build/new/nested\"\n\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("check").output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("project: t"));
}

#[cfg(unix)]
#[test]
fn second_build_reports_up_to_date_and_skips_tool() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho \"$@\" >> build/ran.log\n",
    );
    let first = f.cmd().output().unwrap();
    assert_eq!(first.status.code(), Some(0));
    let second = f.cmd().output().unwrap();
    assert_eq!(second.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&second.stdout).contains("up to date"));
    assert_eq!(
        std::fs::read_to_string(f.path("build/ran.log"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn touched_source_triggers_rebuild() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho x >> build/ran.log\n",
    );
    f.cmd().output().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write("main.tex", "changed");
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(f.path("build/ran.log"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[cfg(unix)]
#[test]
fn deleted_main_with_fresh_pdf_is_config_error() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\n",
    );
    f.cmd().output().unwrap();
    std::fs::remove_file(f.path("main.tex")).unwrap();
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

const FAKE_XELATEX: &str =
    "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho x >> build/ran.log\n";

fn ran_count(f: &Fixture) -> usize {
    std::fs::read_to_string(f.path("build/ran.log"))
        .unwrap_or_default()
        .lines()
        .count()
}

#[cfg(unix)]
#[test]
fn config_mtime_triggers_rebuild() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write_toml(
        "[project]\nname = \"fixture\"\nmain = \"main.tex\"\n\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    assert_eq!(ran_count(&f), 2);
}

#[cfg(unix)]
#[test]
fn unrelated_root_file_triggers_rebuild() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write("notes.txt", "new");
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    assert_eq!(ran_count(&f), 2);
}

#[cfg(unix)]
#[test]
fn file_inside_output_dir_does_not_trigger_rebuild() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write("build/extra.txt", "ignored");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("up to date"));
    assert_eq!(ran_count(&f), 1);
}

#[cfg(unix)]
#[test]
fn git_directory_does_not_trigger_rebuild() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.write(".git/HEAD", "ref: refs/heads/main\n");
    f.cmd().output().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    f.write(".git/index", "ignored");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("up to date"));
    assert_eq!(ran_count(&f), 1);
}

#[cfg(unix)]
#[test]
fn force_flag_rebuilds_an_up_to_date_project() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    let out = f.cmd().arg("-B").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("up to date"));
    assert_eq!(ran_count(&f), 2);
}

#[cfg(unix)]
#[test]
fn recipe_without_latex_always_runs() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\nsteps = [{ run = \"echo x >> ran.log\" }]\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    let ran = std::fs::read_to_string(f.path("ran.log")).unwrap();
    assert_eq!(ran.lines().count(), 2);
}

#[cfg(unix)]
#[test]
fn dry_run_prints_commands_when_up_to_date() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    let out = f.cmd().arg("-n").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("-output-directory=build"), "{stdout}");
    assert!(!stdout.contains("up to date"), "{stdout}");
    assert_eq!(ran_count(&f), 1);
}

#[cfg(unix)]
#[test]
fn build_runs_steps_and_writes_log() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho hello-from-tool\nexit 0\n",
    );
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(f.path("build/main.pdf").exists());
    let log = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    assert!(log.contains("-output-directory=build"), "{log}");
    assert!(log.contains("hello-from-tool"), "{log}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("Built"));
}

/// Assert that `stdout` has one line starting with `prefix` and closing with
/// ` done (x.xs)`. With `-v` the prefix also appears on its own start line, so
/// only completion lines are considered.
#[cfg(unix)]
fn assert_done_line(stdout: &str, prefix: &str) {
    let rest = stdout
        .lines()
        .filter(|line| line.starts_with(prefix))
        .map(|line| &line[prefix.len()..])
        .find(|rest| rest.starts_with(" done ("))
        .unwrap_or_else(|| panic!("missing progress line {prefix:?} in: {stdout}"));
    assert!(rest.ends_with("s)"), "{rest:?}");
    assert_eq!(rest.matches('.').count(), 1, "one decimal place: {rest:?}");
}

#[cfg(unix)]
#[test]
fn progress_numbers_steps_and_reports_elapsed_time() {
    let f = Fixture::new();
    fake_pdf_tool(&f);
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }, { latex = \"xelatex\" }]\n",
    );

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_done_line(&stdout, "[1/2] xelatex main");
    assert_done_line(&stdout, "[2/2] xelatex main");
    let progress = stdout.find("[1/2]").unwrap();
    let built = stdout.find("Built build/main.pdf").unwrap();
    assert!(progress < built, "{stdout}");
}

#[cfg(unix)]
#[test]
fn progress_labels_each_kind_of_step() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\n",
    );
    f.fake_tool("biber", "#!/bin/sh\nexit 0\n");
    f.fake_tool("plantuml", "#!/bin/sh\nexit 0\n");
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n\
         [tools.uml]\ncommand = \"plantuml\"\n\n\
         [recipes.release]\nsteps = [\n\
         { latex = \"xelatex\" },\n\
         { biber = true },\n\
         { tool = \"uml\" },\n\
         { run = \"plantuml -version\" },\n]\n",
    );

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_done_line(&stdout, "[1/4] xelatex main");
    assert_done_line(&stdout, "[2/4] biber main");
    assert_done_line(&stdout, "[3/4] tool plantuml");
    assert_done_line(&stdout, "[4/4] run plantuml");
}

#[cfg(unix)]
#[test]
fn progress_failure_ends_the_line_before_the_report() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\necho boom\nexit 1\n");
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }, { run = \"touch later.marker\" }]\n",
    );

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("[1/2] xelatex main"))
        .unwrap_or_else(|| panic!("missing failure progress line in: {stdout}"));
    assert!(line.contains(" failed ("), "{line:?}");
    assert!(line.ends_with("s)"), "{line:?}");
    assert!(stdout.ends_with('\n'), "{stdout:?}");
    assert!(!stdout.contains("error:"), "{stdout}");
    assert!(!stdout.contains("[2/2]"), "{stdout}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("error: step failed\n"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn quiet_suppresses_progress_lines() {
    let f = Fixture::new();
    fake_pdf_tool(&f);

    let out = f.cmd().args(["-q", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[cfg(unix)]
#[test]
fn dry_run_prints_no_progress_lines() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\nexit 0\n");

    let out = f.cmd().args(["-n", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("done ("), "{stdout}");
    assert!(!stdout.contains('['), "{stdout}");
    assert!(stdout.contains("xelatex"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn verbose_progress_prints_a_complete_completion_line() {
    let f = Fixture::new();
    fake_pdf_tool(&f);
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }, { latex = \"xelatex\" }]\n",
    );

    let out = f.cmd().args(["-v", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_done_line(&stdout, "[1/2] xelatex main");
    assert_done_line(&stdout, "[2/2] xelatex main");
    assert!(
        !stdout.lines().any(|line| line.starts_with(" done (")),
        "no orphan completion line: {stdout}"
    );
    assert!(stdout.contains("$ xelatex"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn verbose_progress_prints_a_complete_failure_line() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\necho boom\nexit 1\n");

    let out = f.cmd().args(["-v", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .filter(|line| line.starts_with("[1/1] xelatex main"))
        .find(|line| line.contains(" failed ("))
        .unwrap_or_else(|| panic!("missing verbose failure progress line in: {stdout}"));
    assert!(line.ends_with("s)"), "{line:?}");
    assert!(
        !stdout.lines().any(|line| line.starts_with(" failed (")),
        "{stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("error: step failed\n"), "{stderr}");
}

#[test]
fn workspace_progress_restarts_numbering_per_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    f.write(
        "paper/texmake.toml",
        "[project]\nname = \"paper\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }, { latex = \"xelatex\" }]\n",
    );
    fake_pdf_tool(&f);

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_done_line(&stdout, "[1/2] xelatex main");
    assert_done_line(&stdout, "[2/2] xelatex main");
    assert_done_line(&stdout, "[1/1] xelatex main");
    assert_eq!(stdout.matches("[1/2]").count(), 1, "{stdout}");
    assert_eq!(stdout.matches("[2/2]").count(), 1, "{stdout}");
}

#[cfg(unix)]
#[test]
fn log_records_command_and_exit_code() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho done\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    let log = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    assert!(log.contains("$ xelatex"), "{log}");
    assert!(log.contains("[0]"), "{log}");
}

#[cfg(unix)]
#[test]
fn missing_tool_exits_1_and_lists_tools() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.release]\nsteps = [{ latex = \"xelatex\" }, { biber = true }]\n",
    );
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("xelatex") && err.contains("biber"), "{err}");
    assert!(f.path("build/texmake.log").exists());
    assert!(
        std::fs::read_to_string(f.path("build/texmake.log"))
            .unwrap()
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn non_executable_tool_is_missing() {
    let f = Fixture::new();
    let bin = f.path("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("xelatex"), "#!/bin/sh\n").unwrap();
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("xelatex"), "{err}");
}

#[cfg(unix)]
#[test]
fn run_steps_are_not_tool_checked() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n[recipes.a]\nsteps = [{ run = \"true\" }]\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
}

#[cfg(unix)]
#[test]
fn tool_with_space_in_path_executes() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("tools dir")).unwrap();
    std::fs::write(
        f.path("tools dir/my tool.sh"),
        "#!/bin/sh\ntouch build/graph.pdf\n",
    )
    .unwrap();
    std::process::Command::new("chmod")
        .args(["+x", f.path("tools dir/my tool.sh").to_str().unwrap()])
        .status()
        .unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\n\
         [tools.graphviz]\ncommand = \"tools dir/my tool.sh\"\nargs = [\"{root}/graph.dot\"]\n\n\
         [recipes.a]\nsteps = [{ tool = \"graphviz\" }]\n",
    );
    let out = f.cmd().output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(f.path("build/graph.pdf").exists());
}

#[cfg(unix)]
#[test]
fn makeindex_runs_in_output_dir() {
    let f = Fixture::new();
    f.fake_tool("makeindex", "#!/bin/sh\ntouch made.index\n");
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n[recipes.a]\nsteps = [{ makeindex = true }]\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(0));
    assert!(f.path("build/made.index").exists());
}

#[cfg(unix)]
#[test]
fn run_step_inherits_environment() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n[recipes.a]\nsteps = [{ run = \"echo $MARKER > env.txt\" }]\n",
    );
    let out = f.cmd().env("MARKER", "ok").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(f.path("env.txt")).unwrap().trim(),
        "ok"
    );
}

#[cfg(unix)]
#[test]
fn failure_reports_command_code_tail_and_log() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nfor i in $(seq 1 30); do echo line-$i; done\nexit 3\n",
    );
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("command:"), "{err}");
    assert!(err.contains("exit code: 3"), "{err}");
    assert!(err.contains("log:"), "{err}");
    assert!(err.contains("line-30"), "{err}");
    assert!(err.contains("line-11"), "{err}");
    assert!(!err.contains("line-10\n"), "{err}");
}

#[cfg(unix)]
#[test]
fn failure_report_has_fixed_format() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\nsteps = [{ run = \"echo one; exit 3\" }]\n",
    );
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    let prefix = "error: step failed\n  command: echo one; exit 3\n  exit code: 3\n  \
                  last 20 lines:\n    one\n  log: ";
    assert!(err.starts_with(prefix), "{err}");
    assert!(err.trim_end().ends_with("build/texmake.log"), "{err}");
}

#[cfg(unix)]
#[test]
fn log_keeps_raw_non_utf8_output_bytes() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\nprintf '\\377\\376' >&2\nexit 1\n");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let log = std::fs::read(f.path("build/texmake.log")).unwrap();
    assert!(
        log.windows(2)
            .any(|pair| pair[0] == 0xff && pair[1] == 0xfe),
        "the log must keep the raw output bytes: {log:?}"
    );
    assert!(
        !log.windows(3)
            .any(|triple| triple[0] == 0xef && triple[1] == 0xbf && triple[2] == 0xbd),
        "the log must not contain lossy UTF-8 replacements: {log:?}"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("\u{fffd}\u{fffd}"), "{err}");
}

#[cfg(unix)]
#[test]
fn verbose_streams_tool_output() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho streamed-line\n",
    );
    let out = f.cmd().arg("-v").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("streamed-line"));
}

#[cfg(unix)]
#[test]
fn verbose_prints_command_and_still_writes_log() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\necho streamed-line\n",
    );
    let out = f.cmd().arg("-v").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("$ xelatex"), "{stdout}");
    assert!(stdout.contains("Built build/main.pdf"), "{stdout}");
    let log = std::fs::read_to_string(f.path("build/texmake.log")).unwrap();
    assert!(log.contains("streamed-line"), "{log}");
}

#[cfg(unix)]
#[test]
fn quiet_suppresses_success_output() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    let out = f.cmd().arg("-q").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("Built"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn quiet_still_prints_failure_report() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\necho boom\nexit 1\n");
    let out = f.cmd().arg("-q").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("boom"));
}

#[cfg(unix)]
#[test]
fn failure_stops_later_steps() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\nexit 1\n");
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }, { run = \"touch later.marker\" }]\n",
    );
    assert_eq!(f.cmd().output().unwrap().status.code(), Some(1));
    assert!(!f.path("later.marker").exists());
}

#[cfg(unix)]
#[test]
fn signal_termination_reports_signal() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\nkill -TERM $$\n");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("signal"));
}

#[cfg(unix)]
#[test]
fn signal_exit_status_is_reported() {
    let f = Fixture::new();
    f.fake_tool("xelatex", "#!/bin/sh\nkill -TERM $$\n");
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("exit code: signal 15"), "{err}");
    assert!(err.contains("terminated by signal 15"), "{err}");
}

#[cfg(unix)]
#[test]
fn pdf_directory_is_not_up_to_date() {
    let f = Fixture::new();
    f.fake_tool("xelatex", FAKE_XELATEX);
    f.cmd().output().unwrap();
    std::fs::remove_file(f.path("build/main.pdf")).unwrap();
    std::fs::create_dir(f.path("build/main.pdf")).unwrap();
    let out = f.cmd().output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Built"));
    assert_eq!(ran_count(&f), 2);
}

#[cfg(unix)]
#[test]
fn clean_removes_output_dir() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\n",
    );
    f.cmd().output().unwrap();
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!f.path("build").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("removed build"));
}

#[cfg(unix)]
#[test]
fn clean_removes_two_outputs_and_nested_parent_only() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\noutput = \"out\"\nsteps = [{ latex = \"xelatex\" }]\n\
         [recipes.b]\noutput = \"out/nested\"\nsteps = [{ latex = \"xelatex\" }]\n\
         [recipes.c]\noutput = \"build\"\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    std::fs::create_dir_all(f.path("out/nested")).unwrap();
    std::fs::create_dir_all(f.path("build")).unwrap();
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!f.path("out").exists());
    assert!(!f.path("build").exists());
}

#[test]
fn clean_dot_output_refused_and_root_intact() {
    let f = Fixture::new();
    f.write_toml("[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \".\"\n[recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n");
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(f.path("texmake.toml").exists());
}

#[cfg(unix)]
#[test]
fn clean_refuses_symlinked_output_outside_root() {
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), f.path("build")).unwrap();
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(outside.path().exists());
}

#[cfg(unix)]
#[test]
fn clean_does_not_follow_symlinks_inside_output() {
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(f.path("build")).unwrap();
    std::os::unix::fs::symlink(outside.path(), f.path("build/link")).unwrap();
    assert_eq!(
        f.cmd().arg("clean").output().unwrap().status.code(),
        Some(0)
    );
    assert!(!f.path("build").exists());
    assert!(outside.path().exists());
}

#[cfg(unix)]
#[test]
fn clean_dry_run_keeps_directories() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("build")).unwrap();
    let out = f.cmd().args(["-n", "clean"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(f.path("build").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("would remove"));
}

#[test]
fn clean_refuses_output_outside_root() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\noutput = \"../out\"\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("outside the project root"), "{err}");
    assert!(f.path("texmake.toml").exists());
}

#[test]
fn clean_refuses_regular_file_output() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [recipes.a]\noutput = \"out.pdf\"\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    std::fs::write(f.path("out.pdf"), "not a directory").unwrap();
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("file, not a directory"), "{err}");
    assert!(f.path("out.pdf").is_file());
}

#[test]
fn clean_skips_missing_output_directories() {
    let f = Fixture::new();
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("removed"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn clean_quiet_suppresses_removed_output() {
    let f = Fixture::new();
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nmkdir -p build && touch build/main.pdf\n",
    );
    f.cmd().output().unwrap();
    let out = f.cmd().args(["-q", "clean"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!f.path("build").exists());
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[cfg(unix)]
#[test]
fn clean_refuses_symlinked_output_inside_root() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("real")).unwrap();
    std::os::unix::fs::symlink(f.path("real"), f.path("linked")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"linked\"\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().arg("clean").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("symbolic link"), "{err}");
    assert!(err.contains("linked"), "{err}");
    assert!(
        f.path("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(f.path("real").is_dir());
}

#[cfg(unix)]
#[test]
fn repeated_clean_still_refuses_symlinked_output() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("real")).unwrap();
    std::os::unix::fs::symlink(f.path("real"), f.path("linked")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"linked\"\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    assert_eq!(
        f.cmd().arg("clean").output().unwrap().status.code(),
        Some(2)
    );
    assert_eq!(
        f.cmd().arg("clean").output().unwrap().status.code(),
        Some(2)
    );
    assert!(
        f.path("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(f.path("real").is_dir());
}

#[cfg(unix)]
#[test]
fn clean_dry_run_refuses_symlinked_output_inside_root() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("real")).unwrap();
    std::os::unix::fs::symlink(f.path("real"), f.path("linked")).unwrap();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\noutput = \"linked\"\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );
    let out = f.cmd().args(["-n", "clean"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("would remove"), "{stdout}");
    assert!(
        f.path("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(f.path("real").is_dir());
}

#[cfg(unix)]
#[test]
fn init_requires_a_terminal() {
    let f = Fixture::new();
    let out = f.cmd().arg("init").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("init needs a terminal"), "{stderr}");
}

#[test]
fn workspace_build_builds_every_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    fake_pdf_tool(&f);

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(f.path("paper/build/main.pdf").is_file());
    assert!(f.path("slides/build/main.pdf").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("==> slides"), "{stdout}");
}

#[test]
fn workspace_build_stops_at_the_first_failing_member() {
    let f = Fixture::new();
    write_workspace(&f, &["a", "b", "c"]);
    write_member(&f, "a", "a");
    f.write(
        "b/texmake.toml",
        "[project]\nname = \"b\"\nmain = \"main.tex\"\n[recipes.release]\nsteps = [{ run = \"exit 7\" }]\n",
    );
    f.write("b/main.tex", "\\documentclass{article}\n");
    write_member(&f, "c", "c");
    fake_pdf_tool(&f);

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    assert!(f.path("a/build/main.pdf").is_file());
    assert!(!f.path("c/build").exists());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member b:"), "{stderr}");
    assert!(stderr.contains("exit code: 7"), "{stderr}");
}

#[test]
fn workspace_check_prints_every_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("==> slides"), "{stdout}");
    assert!(stdout.contains("project: paper"), "{stdout}");
    assert!(stdout.contains("project: slides"), "{stdout}");
}

#[test]
fn workspace_check_reports_an_invalid_member_with_its_path() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    f.write(
        "slides/texmake.toml",
        "[project]\nname = \"slides\"\nmain = \"main.tex\"\nbogus = 1\n[recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n",
    );

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member slides:"), "{stderr}");
    assert!(stderr.contains("slides/texmake.toml"), "{stderr}");
}

#[test]
fn workspace_clean_removes_every_member_output() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    f.write("paper/build/main.pdf", "pdf");
    f.write("slides/build/main.pdf", "pdf");

    let out = f.cmd().arg("clean").output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(!f.path("paper/build").exists());
    assert!(!f.path("slides/build").exists());
    assert!(f.path("paper/main.tex").is_file());
    assert!(f.path("slides/main.tex").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("==> slides"), "{stdout}");
}

#[test]
fn workspace_rejects_an_absolute_member() {
    let f = Fixture::new();
    write_workspace(&f, &["/tmp/elsewhere"]);

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("must be a relative path"), "{stderr}");
}

#[test]
fn workspace_rejects_a_member_outside_the_root() {
    let f = Fixture::new();
    write_workspace(&f, &["../elsewhere"]);

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must stay inside the workspace root"),
        "{stderr}"
    );
}

#[test]
fn workspace_rejects_duplicate_members() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "./paper"]);
    write_member(&f, "paper", "paper");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("duplicate workspace member"), "{stderr}");
}

#[test]
fn workspace_requires_each_member_configuration() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    f.write("paper/main.tex", "\\documentclass{article}\n");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("has no texmake.toml"), "{stderr}");
}

#[test]
fn workspace_rejects_nested_workspaces() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    f.write("paper/texmake.toml", "[workspace]\nmembers = []\n");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("nested workspaces are not supported"),
        "{stderr}"
    );
    assert!(stderr.contains("member paper:"), "{stderr}");
}

#[test]
fn an_empty_workspace_refuses_to_build() {
    let f = Fixture::new();
    write_workspace(&f, &[]);

    let out = f.cmd().arg("build").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("has no members"), "{stderr}");
}

#[test]
fn project_and_workspace_tables_conflict() {
    let f = Fixture::new();
    f.write_toml(
        "[project]\nname = \"t\"\nmain = \"main.tex\"\n\
         [workspace]\nmembers = []\n\
         [recipes.a]\nsteps = [{ latex = \"xelatex\" }]\n",
    );

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot define both"), "{stderr}");
}

#[test]
fn workspace_quiet_suppresses_the_member_headings() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper", "paper");

    let out = f.cmd().args(["-q", "check"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("==> paper"), "{stdout}");
    // `check` always reports the member; only the heading is quiet.
    assert!(stdout.contains("project: paper"), "{stdout}");
}

#[test]
fn workspace_dry_run_prints_member_commands_without_building() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper", "paper");

    let out = f.cmd().args(["-n", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("xelatex"), "{stdout}");
    assert!(!f.path("paper/build").exists());
}

#[cfg(unix)]
#[test]
fn workspace_rejects_a_symlink_member_that_escapes_the_root() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("texmake.toml"),
        "[project]\nname = \"outside\"\nmain = \"main.tex\"\n[recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n",
    )
    .unwrap();
    std::fs::write(
        outside.path().join("main.tex"),
        "\\documentclass{article}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), f.path("paper")).unwrap();

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member `paper`:"), "{stderr}");
    assert!(
        stderr.contains("resolves outside the workspace root"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&outside.path().canonicalize().unwrap().display().to_string()),
        "the error must name the resolved path: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn workspace_accepts_a_symlink_to_a_project_inside_the_root() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper-src", "paper");
    std::os::unix::fs::symlink(f.path("paper-src"), f.path("paper")).unwrap();

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("project: paper"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn workspace_rejects_a_dangling_symlink_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    std::os::unix::fs::symlink(f.path("nowhere"), f.path("paper")).unwrap();

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member `paper`:"), "{stderr}");
    assert!(stderr.contains("cannot resolve"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn workspace_rejects_a_symlink_loop_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    std::os::unix::fs::symlink(f.path("paper"), f.path("paper")).unwrap();

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member `paper`:"), "{stderr}");
    assert!(stderr.contains("cannot resolve"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn workspace_rejects_a_symlinked_config_that_escapes_the_root() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    f.write("paper/main.tex", "\\documentclass{article}\n");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("texmake.toml"),
        "[project]\nname = \"outside\"\nmain = \"main.tex\"\n[recipes.release]\nsteps = [{ latex = \"xelatex\" }]\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("texmake.toml"),
        f.path("paper/texmake.toml"),
    )
    .unwrap();

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member `paper`:"), "{stderr}");
    assert!(
        stderr.contains("resolves outside the workspace root"),
        "{stderr}"
    );
}

#[test]
fn workspace_rejects_a_member_that_is_not_a_directory() {
    let f = Fixture::new();
    write_workspace(&f, &["file.txt"]);
    f.write("file.txt", "not a directory\n");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("member `file.txt`"), "{stderr}");
    assert!(stderr.contains("is not a directory"), "{stderr}");
}

#[test]
fn workspace_force_rebuilds_every_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    fake_counting_tool(&f);

    f.cmd().arg("build").output().unwrap();
    assert_eq!(run_count(&f), 2, "the first build must run every member");

    let out = f.cmd().arg("build").output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert_eq!(run_count(&f), 2, "an up-to-date workspace must not rebuild");

    let out = f.cmd().args(["-B", "build"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert_eq!(run_count(&f), 4, "-B must rebuild every member");
}

#[test]
fn workspace_accepts_a_file_option_from_another_directory() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper", "paper");
    std::fs::create_dir_all(f.path("sub")).unwrap();

    let out = cmd_at(&f, &f.path("sub"))
        .args(["-f", "../texmake.toml", "check"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("project: paper"), "{stdout}");
}

#[test]
fn workspace_verbose_streams_member_output() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper", "paper");
    f.fake_tool(
        "xelatex",
        "#!/bin/sh\nout=build\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    -output-directory=*) out=\"${arg#-output-directory=}\";;\n  esac\ndone\nmkdir -p \"$out\"\nprintf 'pdf' > \"$out/main.pdf\"\necho \"hello from paper\"\n",
    );

    let out = f.cmd().args(["-v", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    let heading = stdout.find("==> paper").unwrap();
    let streamed = stdout
        .find("hello from paper")
        .unwrap_or_else(|| panic!("the tool output must be streamed: {stdout}"));
    assert!(heading < streamed, "{stdout}");
}

#[test]
fn workspace_clean_dry_run_prints_and_keeps_outputs() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    f.write("paper/build/main.pdf", "pdf");
    f.write("slides/build/main.pdf", "pdf");

    let out = f.cmd().args(["-n", "clean"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(f.path("paper/build/main.pdf").is_file());
    assert!(f.path("slides/build/main.pdf").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(stdout.contains("would remove build"), "{stdout}");
}

#[test]
fn workspace_quiet_clean_prints_nothing() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    f.write("paper/build/main.pdf", "pdf");
    f.write("slides/build/main.pdf", "pdf");

    let out = f.cmd().args(["-q", "clean"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(!f.path("paper/build").exists());
    assert!(!f.path("slides/build").exists());
    assert!(
        String::from_utf8_lossy(&out.stdout).is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

// ---------------------------------------------------------------------------
// Workspace member selection with -p / --project
// ---------------------------------------------------------------------------

#[test]
fn workspace_build_with_one_project_builds_only_that_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    fake_pdf_tool(&f);

    let out = f.cmd().args(["-p", "paper", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(f.path("paper/build/main.pdf").is_file());
    assert!(!f.path("slides/build").exists());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(!stdout.contains("==> slides"), "{stdout}");
}

#[test]
fn workspace_build_with_two_projects_keeps_member_order() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    fake_pdf_tool(&f);

    // Selected in reverse order, executed in `members` order.
    let out = f
        .cmd()
        .args(["-p", "slides", "-p", "paper", "build"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(f.path("paper/build/main.pdf").is_file());
    assert!(f.path("slides/build/main.pdf").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let paper = stdout
        .find("==> paper")
        .unwrap_or_else(|| panic!("paper must be built: {stdout}"));
    let slides = stdout
        .find("==> slides")
        .unwrap_or_else(|| panic!("slides must be built: {stdout}"));
    assert!(paper < slides, "members order must be kept: {stdout}");
}

#[test]
fn workspace_repeated_project_flags_select_the_member_once() {
    let f = Fixture::new();
    write_workspace(&f, &["paper"]);
    write_member(&f, "paper", "paper");
    fake_counting_tool(&f);

    let out = f
        .cmd()
        .args(["-p", "paper", "-p", "paper", "build"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert_eq!(run_count(&f), 1, "a repeated -p value must not build twice");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.matches("==> paper").count(), 1, "{stdout}");
}

#[test]
fn workspace_unknown_project_lists_the_available_names() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");

    let out = f.cmd().args(["-p", "nope", "check"]).output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown project `nope`"), "{stderr}");
    assert!(stderr.contains("available: paper, slides"), "{stderr}");
}

#[test]
fn project_flag_is_rejected_in_a_single_project() {
    let f = Fixture::new();

    let out = f.cmd().args(["-p", "paper", "build"]).output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("only valid in a workspace"), "{stderr}");
}

#[test]
fn project_flag_is_rejected_by_init() {
    let f = Fixture::new();

    let out = f.cmd().args(["-p", "x", "init"]).output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("only valid in a workspace"), "{stderr}");
}

#[test]
fn workspace_rejects_duplicate_member_project_names() {
    let f = Fixture::new();
    write_workspace(&f, &["a", "b"]);
    write_member(&f, "a", "paper");
    write_member(&f, "b", "paper");

    let out = f.cmd().arg("check").output().unwrap();

    assert_eq!(out.status.code(), Some(2), "{:?}", out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("duplicate project name `paper`"),
        "{stderr}"
    );
    assert!(stderr.contains("a/texmake.toml"), "{stderr}");
    assert!(stderr.contains("b/texmake.toml"), "{stderr}");
}

#[test]
fn workspace_check_with_project_selects_one_member() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");

    let out = f.cmd().args(["-p", "paper", "check"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(!stdout.contains("==> slides"), "{stdout}");
    assert!(!stdout.contains("project: slides"), "{stdout}");
}

#[test]
fn workspace_clean_with_project_keeps_the_other_outputs() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");
    f.write("paper/build/main.pdf", "pdf");
    f.write("slides/build/main.pdf", "pdf");

    let out = f.cmd().args(["-p", "paper", "clean"]).output().unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(!f.path("paper/build").exists());
    assert!(f.path("slides/build/main.pdf").is_file());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(!stdout.contains("==> slides"), "{stdout}");
}

#[test]
fn workspace_dry_run_with_project_prints_only_the_selected_steps() {
    let f = Fixture::new();
    write_workspace(&f, &["paper", "slides"]);
    write_member(&f, "paper", "paper");
    write_member(&f, "slides", "slides");

    let out = f
        .cmd()
        .args(["-n", "-p", "paper", "build"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0), "{:?}", out);
    assert!(!f.path("paper/build").exists());
    assert!(!f.path("slides/build").exists());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("==> paper"), "{stdout}");
    assert!(!stdout.contains("==> slides"), "{stdout}");
    assert_eq!(stdout.matches("xelatex").count(), 1, "{stdout}");
}
