mod helpers;

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
            "--task",
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
