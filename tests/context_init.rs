mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_context_init_empty_by_default() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    // Create some spec files (these should NOT be auto-discovered)
    let spec_dir = env.root().join(".mem").join("main").join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("index.md"), "# Index")?;
    fs::write(spec_dir.join("plan.md"), "# Plan")?;

    // Run mem context init
    env.command()
        .arg("context")
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .mem/main/context.json"));

    // Verify content: artifacts should be empty even though spec files exist
    let context_json = env.root().join(".mem").join("main").join("context.json");
    let content = fs::read_to_string(context_json)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;

    assert!(v["default"]["artifacts"].is_array());
    let artifacts = v["default"]["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 0);

    Ok(())
}

#[test]
fn test_context_init_force_overwrites() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    let context_json = env.root().join(".mem").join("main").join("context.json");
    fs::create_dir_all(context_json.parent().unwrap())?;
    fs::write(&context_json, "{}")?;

    // Run without force should fail
    env.command()
        .arg("context")
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // Run with force should succeed
    env.command()
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
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Create mem.json with context template
    let mem_json = env.root().join("mem.json");
    fs::write(
        &mem_json,
        r#"{
        "context": {
            "default": {
                "artifacts": ["./spec/index.md", "./spec/tickets/*"]
            }
        }
    }"#,
    )?;

    // Initialize mem
    env.command().arg("init").assert().success();

    // Run mem context init
    env.command().arg("context").arg("init").assert().success();

    // Verify content matches template
    let context_json = env.root().join(".mem").join("main").join("context.json");
    let content = fs::read_to_string(context_json)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;

    assert_eq!(v["default"]["artifacts"][0], "./spec/index.md");
    assert_eq!(v["default"]["artifacts"][1], "./spec/tickets/*");
    assert!(v["default"]["diff"].is_null());

    Ok(())
}
