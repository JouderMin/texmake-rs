mod common;

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
