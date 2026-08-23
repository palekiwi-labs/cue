//! Integration tests for spec validation through the `cue-agent`
//! binary: every spec error exits 2 with a message on stderr and
//! nothing on stdout.

fn cmd() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("cue-agent").expect("cue-agent binary")
}

#[test]
fn invalid_json_exits_2() {
    cmd()
        .args(["run", "[not json]"])
        .assert()
        .failure()
        .code(2)
        .stdout("");
}

#[test]
fn not_an_array_exits_2() {
    cmd()
        .args(["run", r#"{"prompt": "x"}"#])
        .assert()
        .failure()
        .code(2)
        .stdout("");
}

#[test]
fn empty_array_exits_2() {
    cmd()
        .args(["run", "[]"])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("empty"));
}

#[test]
fn unknown_field_exits_2() {
    cmd()
        .args(["run", r#"[{"prompt": "x", "bogus": 1}]"#])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("unknown field"));
}

#[test]
fn prompt_required_exits_2() {
    cmd()
        .args(["run", r#"[{"model": "m"}]"#])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("prompt is required"));
}

#[test]
fn duplicate_ids_exit_2() {
    let spec = r#"[{"id": "a", "prompt": "x"}, {"id": "a", "prompt": "y"}]"#;
    cmd()
        .args(["run", spec])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("duplicate run id 'a'"));
}

#[test]
fn background_true_exits_2() {
    cmd()
        .args(["run", r#"[{"prompt": "x", "background": true}]"#])
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains("background"));
}

#[test]
fn kitchen_sink_spec_exits_0() {
    let spec = r#"[{
        "id": "reviewer-a",
        "model": "google/gemini-3.6-flash",
        "system-prompt": "You are an auditor.",
        "append-system-prompt": ["Focus on hot paths."],
        "prompt": "Review the diff.",
        "tools": ["read", "grep"],
        "thinking": "medium",
        "approve": true,
        "session": {"persist": true, "id": "4f9a2c1e-0000-7000-8000-000000000000"},
        "env": {"GEMINI_API_KEY": "secret"},
        "worktree": {"mode": "ephemeral", "base": "main"},
        "timeout": 300
    }]"#;
    cmd().arg("run").arg(spec).assert().success().stdout("");
}

#[test]
fn file_refs_resolve_relative_to_spec_file_dir_not_cwd() {
    let dir = tempfile::tempdir().expect("tempdir");
    let subdir = dir.path().join("specs");
    std::fs::create_dir(&subdir).expect("mkdir");
    std::fs::write(subdir.join("prompt.txt"), "prompt from sibling file").expect("write");
    std::fs::write(
        subdir.join("spec.json"),
        r#"[{"prompt": {"file": "prompt.txt"}}]"#,
    )
    .expect("write spec");

    // Run from a cwd that does NOT contain prompt.txt: the {file} ref
    // must resolve against the spec file's directory.
    cmd()
        .args(["run", "--spec-file"])
        .arg(subdir.join("spec.json"))
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout("");
}

#[test]
fn missing_file_ref_exits_2_naming_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let spec_path = dir.path().join("spec.json");
    std::fs::write(&spec_path, r#"[{"prompt": {"file": "nope.txt"}}]"#).expect("write spec");

    let expected = dir.path().join("nope.txt");
    cmd()
        .args(["run", "--spec-file"])
        .arg(&spec_path)
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicates::str::contains(
            expected.to_string_lossy().as_ref(),
        ));
}
