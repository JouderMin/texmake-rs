mod common;

use common::Fixture;

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
