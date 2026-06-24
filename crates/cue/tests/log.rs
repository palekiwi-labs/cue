mod helpers;

use predicates::prelude::*;
use std::fs;

#[test]
fn test_log_add_basic() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a log entry
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Test Title")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/log.md\n"));

    let log_path = env.root().join(".test-mem/main/spec/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("# Project Log"));
    assert!(content.contains("Test Title"));

    // Add another log entry with dirty tree
    fs::write(env.root().join("dirty.txt"), "dirty")?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Dirty Entry")
        .arg("--body")
        .arg("Some body text")
        .arg("--found")
        .arg("Found something")
        .arg("--decided")
        .arg("Decided something")
        .assert()
        .success();

    let content = fs::read_to_string(&log_path)?;
    assert!(content.contains("-dirty] Dirty Entry"));
    assert!(content.contains("Some body text"));
    assert!(content.contains("- **Found:** Found something"));
    assert!(content.contains("- **Decided:** Decided something"));

    Ok(())
}

#[test]
fn test_log_add_from_file() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let json_content = r#"{
        "title": "JSON Title",
        "body": "JSON Body",
        "open": ["Question 1", "Question 2"]
    }"#;
    let json_path = env.root().join("log.json");
    fs::write(&json_path, json_content)?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--file")
        .arg(&json_path)
        .assert()
        .success();

    let log_path = env.root().join(".test-mem/main/spec/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("JSON Title"));
    assert!(content.contains("JSON Body"));
    assert!(content.contains("- **Open:** Question 1"));
    assert!(content.contains("- **Open:** Question 2"));

    Ok(())
}

#[test]
fn test_log_add_validation() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Empty title
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("   ")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Title cannot be empty"));

    // Missing title
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--body")
        .arg("Some body")
        .assert()
        .failure()
        .stderr(predicate::str::contains("The --title argument is required"));

    Ok(())
}

#[test]
fn test_log_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Uninitialized
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Initialized but no log
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Add entry
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("My Title")
        .assert()
        .success();

    // 3. Has log
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Project Log"))
        .stdout(predicate::str::contains("My Title"));

    Ok(())
}

#[test]
fn test_log_add_with_explicit_branch() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a log entry to a DIFFERENT branch than current (main)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Branch Entry")
        .arg("--branch")
        .arg("feature/other")
        .assert()
        .success()
        .stdout(predicate::str::diff(
            ".test-mem/feature-other/spec/log.md\n",
        ));

    let log_path = env.root().join(".test-mem/feature-other/spec/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("# Project Log"));
    assert!(content.contains("Branch Entry"));

    // Verify main branch log does not have this entry
    let main_log = env.root().join(".test-mem/main/spec/log.md");
    assert!(!main_log.exists());

    Ok(())
}
