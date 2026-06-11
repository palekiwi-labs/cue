mod helpers;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_init_fresh_repo() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Initialized .test-mem/ directory"));

    let mem_dir = temp.path().join(".test-mem");
    assert!(mem_dir.exists());
    assert!(mem_dir.join(".gitignore").exists());
    assert!(mem_dir.join(".rgignore").exists());

    // Default gitignore covers the default ignored_types: ["tmp"]
    let gitignore = fs::read_to_string(mem_dir.join(".gitignore"))?;
    assert!(gitignore.contains("*/tmp/"));

    Ok(())
}

#[test]
fn test_init_gitignore_respects_config() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Configure custom ignored types
    fs::write(
        temp.path().join("mem.json"),
        r#"{"ignored_types": ["tmp", "ref"]}"#,
    )?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let gitignore = fs::read_to_string(temp.path().join(".test-mem/.gitignore"))?;
    assert!(gitignore.contains("*/tmp/"));
    assert!(gitignore.contains("*/ref/"));

    let rgignore = fs::read_to_string(temp.path().join(".test-mem/.rgignore"))?;
    assert!(rgignore.contains("!*/tmp/"));
    assert!(rgignore.contains("!*/ref/"));

    Ok(())
}

#[test]
fn test_init_not_a_git_repo() -> anyhow::Result<()> {
    let temp = TempDir::new()?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path()).arg("init");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));

    Ok(())
}

#[test]
fn test_init_already_initialized() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // First init
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Second init
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");

    cmd.assert().success().stdout(predicate::str::contains(
        ".test-mem/ directory already exists. Already initialized?",
    ));

    Ok(())
}

#[test]
fn test_init_local_branch_exists() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Create a local branch but don't leave it checked out in a worktree
    // Note: In a fresh repo, main is checked out.
    // If we create test-mem, it becomes the new branch.
    // We should create it and then switch back to main.
    std::process::Command::new("git")
        .args(["branch", "test-mem"])
        .current_dir(temp.path())
        .output()?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");

    cmd.assert().success();

    let mem_dir = temp.path().join(".test-mem");
    assert!(mem_dir.exists());

    Ok(())
}

#[test]
fn test_init_remote_branch_exists() -> anyhow::Result<()> {
    let temp_local = TempDir::new()?;
    let temp_remote = TempDir::new()?;

    helpers::setup_git_repo(temp_local.path());
    helpers::setup_remote(temp_local.path(), temp_remote.path());

    // Create and push branch
    std::process::Command::new("git")
        .args(["checkout", "-b", "test-mem"])
        .current_dir(temp_local.path())
        .output()?;

    std::process::Command::new("git")
        .args(["push", "origin", "test-mem"])
        .current_dir(temp_local.path())
        .output()?;

    // Delete local branch to simulate "remote only"
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_local.path())
        .output()?;

    std::process::Command::new("git")
        .args(["branch", "-D", "test-mem"])
        .current_dir(temp_local.path())
        .output()?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp_local.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Found test-mem branch on remote"));

    let mem_dir = temp_local.path().join(".test-mem");
    assert!(mem_dir.exists());

    // Verify upstream tracking
    let output = std::process::Command::new("git")
        .args(["config", "branch.test-mem.remote"])
        .current_dir(mem_dir)
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    Ok(())
}
