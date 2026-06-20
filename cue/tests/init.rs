mod helpers;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_init_fresh_repo() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized .test-mem/ directory"));

    let mem_dir = env.root().join(".test-mem");
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
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Configure custom ignored types
    fs::write(
        env.root().join("cue.json"),
        r#"{"ignored_types": ["tmp", "ref"]}"#,
    )?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let gitignore = fs::read_to_string(env.root().join(".test-mem/.gitignore"))?;
    assert!(gitignore.contains("*/tmp/"));
    assert!(gitignore.contains("*/ref/"));

    let rgignore = fs::read_to_string(env.root().join(".test-mem/.rgignore"))?;
    assert!(rgignore.contains("!*/tmp/"));
    assert!(rgignore.contains("!*/ref/"));

    Ok(())
}

#[test]
fn test_init_not_a_git_repo() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));

    Ok(())
}

#[test]
fn test_init_already_initialized() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // First init
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Second init
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            ".test-mem/ directory already exists. Already initialized?",
        ));

    Ok(())
}

#[test]
fn test_init_local_branch_exists() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Create a local branch but don't leave it checked out in a worktree
    std::process::Command::new("git")
        .args(["branch", "test-mem"])
        .current_dir(env.root())
        .output()?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let mem_dir = env.root().join(".test-mem");
    assert!(mem_dir.exists());

    Ok(())
}

#[test]
fn test_init_registers_project_in_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let store_path = env.data_dir.join("projects.json");
    assert!(
        store_path.exists(),
        "projects.json should be created by init"
    );

    let content = fs::read_to_string(&store_path)?;
    assert!(
        content.contains(env.root().to_str().unwrap()),
        "store should contain the project path"
    );

    Ok(())
}

#[test]
fn test_init_twice_does_not_duplicate_entry() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // First init
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Second init — already initialized, but store registration must be idempotent
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let content = fs::read_to_string(env.data_dir.join("projects.json"))?;
    let path_str = env.root().to_str().unwrap();
    let count = content.matches(path_str).count();
    assert_eq!(count, 1, "path should appear exactly once in store");

    Ok(())
}

#[test]
fn test_init_remote_branch_exists() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let temp_remote = TempDir::new()?;

    helpers::setup_git_repo(env.root());
    helpers::setup_remote(env.root(), temp_remote.path());

    // Create and push branch
    std::process::Command::new("git")
        .args(["checkout", "-b", "test-mem"])
        .current_dir(env.root())
        .output()?;

    std::process::Command::new("git")
        .args(["push", "origin", "test-mem"])
        .current_dir(env.root())
        .output()?;

    // Delete local branch to simulate "remote only"
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(env.root())
        .output()?;

    std::process::Command::new("git")
        .args(["branch", "-D", "test-mem"])
        .current_dir(env.root())
        .output()?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found test-mem branch on remote"));

    let mem_dir = env.root().join(".test-mem");
    assert!(mem_dir.exists());

    // Verify upstream tracking
    let output = std::process::Command::new("git")
        .args(["config", "branch.test-mem.remote"])
        .current_dir(&mem_dir)
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    Ok(())
}
