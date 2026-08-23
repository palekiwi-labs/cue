//! Integration tests for the `cue-agent` CLI shell.

/// Spawn an isolated `cue-agent` command.
fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("cue-agent").expect("cue-agent binary")
}

#[test]
fn valid_minimal_spec_exits_0() {
    cmd()
        .args(["run", r#"[{"prompt": "hello"}]"#])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn missing_spec_input_exits_2() {
    cmd()
        .arg("run")
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("spec"));
}

#[test]
fn positional_conflicts_with_spec_file_exits_2() {
    cmd()
        .args(["run", "--spec-file", "spec.json", r#"[{"prompt": "x"}]"#])
        .assert()
        .failure()
        .code(2)
        .stdout("");
}

#[test]
fn stdin_dash_reads_stdin() {
    cmd()
        .arg("run")
        .arg("-")
        .write_stdin(r#"[{"prompt": "from stdin"}]"#)
        .assert()
        .success()
        .stdout("");
}

#[test]
fn spec_file_reads_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let spec_path = dir.path().join("spec.json");
    std::fs::write(&spec_path, r#"[{"prompt": "from file"}]"#).expect("write spec");

    cmd()
        .args(["run", "--spec-file"])
        .arg(&spec_path)
        .assert()
        .success()
        .stdout("");
}

#[test]
fn missing_spec_file_exits_2() {
    cmd()
        .args(["run", "--spec-file", "/nonexistent/spec.json"])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("/nonexistent/spec.json"));
}
