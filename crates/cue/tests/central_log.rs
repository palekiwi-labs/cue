mod helpers;

use predicates::prelude::*;

#[test]
fn log_add_writes_a_structured_json_entry() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
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
    env.command().arg("init").assert().success();
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
fn log_list_requires_an_active_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    env.command()
        .args(["log", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --task <context>",
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
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "log",
            "add",
            "--task",
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
