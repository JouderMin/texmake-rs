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
