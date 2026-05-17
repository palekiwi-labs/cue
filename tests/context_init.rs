mod helpers;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_context_init_auto_populates_spec() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path()).arg("init").assert().success();

    // Create some spec files
    let spec_dir = temp.path().join(".mem").join("main").join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("index.md"), "# Index")?;
    fs::write(spec_dir.join("plan.md"), "# Plan")?;

    // Run mem context init
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("context")
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .mem/main/context.json"));

    // Verify content
    let context_json = temp.path().join(".mem").join("main").join("context.json");
    let content = fs::read_to_string(context_json)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;

    assert!(v["default"]["artifacts"].is_array());
    let artifacts = v["default"]["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 2);
    assert_eq!(artifacts[0], "./spec/index.md");
    assert_eq!(artifacts[1], "./spec/plan.md");

    Ok(())
}

#[test]
fn test_context_init_force_overwrites() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path()).arg("init").assert().success();

    let context_json = temp.path().join(".mem").join("main").join("context.json");
    fs::create_dir_all(context_json.parent().unwrap())?;
    fs::write(&context_json, "{}")?;

    // Run without force should fail
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("context")
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // Run with force should succeed
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("context")
        .arg("init")
        .arg("--force")
        .assert()
        .success();

    let content = fs::read_to_string(&context_json)?;
    assert!(content.contains("default"));

    Ok(())
}

#[test]
fn test_context_init_with_template() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Create mem.json with context template
    let mem_json = temp.path().join("mem.json");
    fs::write(
        &mem_json,
        r#"{
        "context": {
            "default": {
                "artifacts": ["./spec/index.md", "./spec/tickets/*"],
                "diff": "main...HEAD"
            }
        }
    }"#,
    )?;

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path()).arg("init").assert().success();

    // Run mem context init
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("context")
        .arg("init")
        .assert()
        .success();

    // Verify content matches template
    let context_json = temp.path().join(".mem").join("main").join("context.json");
    let content = fs::read_to_string(context_json)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;

    assert_eq!(v["default"]["artifacts"][0], "./spec/index.md");
    assert_eq!(v["default"]["artifacts"][1], "./spec/tickets/*");
    assert_eq!(v["default"]["diff"], "main...HEAD");

    Ok(())
}
