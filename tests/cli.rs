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
