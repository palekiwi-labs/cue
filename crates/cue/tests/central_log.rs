mod helpers;

use predicates::prelude::*;

#[test]
fn log_add_writes_a_structured_json_entry() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Validated release",
            "--found",
            "Smoke test passed",
        ])
        .assert()
        .success();

    let log_dir = env.cue_store().join("acme/widgets/release/log");
    let entries = std::fs::read_dir(&log_dir)?.collect::<Result<Vec<_>, _>>()?;

    assert_eq!(entries.len(), 1);
    let path = entries[0].path();
    let entry: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let timestamp = entry["timestamp"]
        .as_u64()
        .expect("timestamp should be an integer");
    let expected_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env.root())
        .output()?;
    let mut expected_hash = String::from_utf8(expected_hash.stdout)?.trim().to_owned();
    expected_hash.push_str("-dirty");

    assert_eq!(
        path.file_name().and_then(|value| value.to_str()),
        Some(format!("{timestamp:020}.json").as_str())
    );
    assert_eq!(entry["commit_hash"], expected_hash);
    assert_eq!(entry["title"], "Validated release");
    assert!(entry["trace"].is_null());
    assert_eq!(entry["found"], serde_json::json!(["Smoke test passed"]));
    assert_eq!(entry["decided"], serde_json::json!([]));
    assert_eq!(entry["open"], serde_json::json!([]));
    assert!(!env.root().join(".cue").exists());

    Ok(())
}

#[test]
fn log_list_outputs_entries_in_chronological_order() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for title in ["First discovery", "Second discovery"] {
        env.command()
            .args(["log", "add", "--context", "release", "--title", title])
            .assert()
            .success();
    }

    let output = env
        .command()
        .args(["log", "list", "--context", "release"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(entries.as_array().map(Vec::len), Some(2));
    assert_eq!(entries[0]["title"], "First discovery");
    assert_eq!(entries[1]["title"], "Second discovery");
    assert!(entries[0]["timestamp"].as_u64() < entries[1]["timestamp"].as_u64());

    Ok(())
}

#[test]
fn log_list_renders_markdown_in_chronological_order() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for (title, finding) in [
        ("First discovery", "First fact"),
        ("Second discovery", "Second fact"),
    ] {
        env.command()
            .args([
                "log",
                "add",
                "--context",
                "release",
                "--title",
                title,
                "--found",
                finding,
            ])
            .assert()
            .success();
    }

    let json = env
        .command()
        .args(["log", "list", "--context", "release"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&json)?;
    let first_hash = entries[0]["commit_hash"].as_str().unwrap();
    let second_hash = entries[1]["commit_hash"].as_str().unwrap();

    env.command()
        .args([
            "log",
            "list",
            "--context",
            "release",
            "--format",
            "md",
        ])
        .assert()
        .success()
        .stdout(format!(
            "## [{first_hash}] First discovery\n\n- **Found:** First fact\n\n## [{second_hash}] Second discovery\n\n- **Found:** Second fact\n\n"
        ));

    Ok(())
}

#[test]
fn markdown_log_list_renders_trace_links() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "evidence",
            "Release evidence",
            "--type",
            "trace",
            "--context",
            "release",
        ])
        .assert()
        .success();
    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Captured evidence",
            "--trace",
            "acme/widgets/release/trace/evidence.md",
        ])
        .assert()
        .success();

    env.command()
        .args(["log", "list", "--context", "release", "--format", "md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[trace](trace/evidence.md)"));
}

#[test]
fn log_add_rejects_context_relative_trace_references() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "evidence",
            "Release evidence",
            "--type",
            "trace",
            "--context",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Captured evidence",
            "--trace",
            "trace/evidence.md",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Trace reference does not exist: trace/evidence.md",
        ));
}

#[test]
fn log_add_reports_non_trace_artifacts_in_context_vocabulary() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    let context_file = env.cue_store().join("acme/widgets/release/context.md");

    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Invalid trace",
            "--trace",
            context_file.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "must target a trace artifact in context 'release'",
        ));
}

#[test]
fn log_add_reports_traces_outside_the_repository_scope() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    let other_trace = env.cue_store().join("acme/other/release/trace/evidence.md");
    std::fs::create_dir_all(other_trace.parent().unwrap()).unwrap();
    std::fs::write(&other_trace, "evidence").unwrap();

    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Invalid trace",
            "--trace",
            other_trace.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "resolves outside this repository's scope",
        ));
}

#[test]
fn markdown_log_list_is_empty_without_entries() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args(["log", "list", "--context", "release", "--format", "md"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn log_list_limit_keeps_the_newest_json_entries() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for title in ["First discovery", "Second discovery"] {
        env.command()
            .args(["log", "add", "--context", "release", "--title", title])
            .assert()
            .success();
    }

    let output = env
        .command()
        .args(["log", "list", "--context", "release", "--limit", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(entries.as_array().map(Vec::len), Some(1));
    assert_eq!(entries[0]["title"], "Second discovery");

    Ok(())
}

#[test]
fn log_list_limit_keeps_the_newest_markdown_entries() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for title in ["First discovery", "Second discovery"] {
        env.command()
            .args(["log", "add", "--context", "release", "--title", title])
            .assert()
            .success();
    }

    env.command()
        .args([
            "log",
            "list",
            "--context",
            "release",
            "--format",
            "md",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Second discovery")
                .and(predicate::str::contains("First discovery").not()),
        );
}

#[test]
fn log_list_requires_an_active_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["log", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --context <context>",
        ));
}

#[test]
fn log_add_requires_a_repository_revision() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let init = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(env.root())
        .output()?;
    assert!(init.status.success());
    helpers::setup_origin(env.root(), helpers::TEST_ORIGIN_URL);
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Validated release",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Failed to resolve current commit for log entry",
        ));

    Ok(())
}
