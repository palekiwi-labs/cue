mod helpers;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_log_add_basic() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a log entry
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Test Title");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/log.md\n"));

    let log_path = temp.path().join(".test-mem/main/spec/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("# Project Log"));
    assert!(content.contains("Test Title"));

    // Add another log entry with dirty tree
    fs::write(temp.path().join("dirty.txt"), "dirty")?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Dirty Entry")
        .arg("--body")
        .arg("Some body text")
        .arg("--found")
        .arg("Found something")
        .arg("--decided")
        .arg("Decided something");

    cmd.assert().success();

    let content = fs::read_to_string(&log_path)?;
    assert!(content.contains("-dirty] Dirty Entry"));
    assert!(content.contains("Some body text"));
    assert!(content.contains("- **Found:** Found something"));
    assert!(content.contains("- **Decided:** Decided something"));

    Ok(())
}

#[test]
fn test_log_add_from_file() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let json_content = r#"{
        "title": "JSON Title",
        "body": "JSON Body",
        "open": ["Question 1", "Question 2"]
    }"#;
    let json_path = temp.path().join("log.json");
    fs::write(&json_path, json_content)?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--file")
        .arg(&json_path);

    cmd.assert().success();

    let log_path = temp.path().join(".test-mem/main/spec/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("JSON Title"));
    assert!(content.contains("JSON Body"));
    assert!(content.contains("- **Open:** Question 1"));
    assert!(content.contains("- **Open:** Question 2"));

    Ok(())
}

#[test]
fn test_log_add_validation() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Empty title
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("   ");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Title cannot be empty"));

    // Missing title
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--body")
        .arg("Some body");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("The --title argument is required"));

    Ok(())
}

#[test]
fn test_log_list() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Uninitialized
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list");
    cmd.assert().success().stdout(predicate::str::is_empty());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Initialized but no log
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list");
    cmd.assert().success().stdout(predicate::str::is_empty());

    // Add entry
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("My Title");
    cmd.assert().success();

    // 3. Has log
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("# Project Log"))
        .stdout(predicate::str::contains("My Title"));

    Ok(())
}
