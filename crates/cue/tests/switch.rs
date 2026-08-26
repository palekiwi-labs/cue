mod helpers;

use predicates::prelude::*;
use serde_json::Value;

#[test]
fn switch_json_to_task() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("switch")
            .arg("auth-login")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;

    assert_eq!(json["context"], "auth-login");
    assert_eq!(json["global"], false);

    Ok(())
}

#[test]
fn switch_json_to_master() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("switch")
            .arg("master")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;

    assert_eq!(json["context"], "master");
    assert_eq!(json["global"], true);

    Ok(())
}

#[test]
fn switch_traversal_slug_is_rejected() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("../../evil")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid task slug"));

    // No directory should have been created outside .test-mem/
    let escaped = env.root().join("evil");
    assert!(
        !escaped.exists(),
        "traversal must not create a dir above .cue"
    );

    Ok(())
}

#[test]
fn switch_absolute_path_slug_is_rejected() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("/tmp/evil")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid task slug"));

    // The absolute target must not have been created
    assert!(
        !std::path::Path::new("/tmp/evil").exists(),
        "absolute path must not be created"
    );

    Ok(())
}

#[test]
fn switch_human_output_to_task() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"));

    // The context directory must be auto-created.
    let task_dir = env.root().join(".test-mem/auth-login");
    assert!(task_dir.is_dir(), "context directory must be created");

    Ok(())
}

#[test]
fn switch_human_output_to_master() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("master")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to global context\n"));

    Ok(())
}

#[test]
fn switch_in_linked_worktree_updates_local_head_and_main_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Create a linked worktree
    let wt = env.root().join("wt");
    let out = std::process::Command::new("git")
        .args(["worktree", "add", wt.to_str().unwrap(), "-b", "topic"])
        .current_dir(env.root())
        .output()
        .expect("git worktree add failed");
    assert!(
        out.status.success(),
        "git worktree add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Run `cue switch auth-login` from inside the linked worktree
    env.command()
        .current_dir(&wt)
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"));

    // Verify local worktree .test-mem/HEAD is written
    let wt_head = wt.join(".test-mem/HEAD");
    assert!(wt_head.exists(), "worktree HEAD must be written");
    let wt_head_content = std::fs::read_to_string(&wt_head)?;
    assert_eq!(wt_head_content.trim(), "auth-login");

    // Verify main root .test-mem/HEAD is NOT modified
    let main_head = env.root().join(".test-mem/HEAD");
    if main_head.exists() {
        let main_head_content = std::fs::read_to_string(&main_head)?;
        assert_ne!(main_head_content.trim(), "auth-login");
    }

    // Verify task directory created under main root store (.test-mem/auth-login)
    let main_task_dir = env.root().join(".test-mem/auth-login");
    assert!(
        main_task_dir.is_dir(),
        "task context directory must be created in main store"
    );

    // Verify NO local scope leakage (worktree does NOT have .test-mem/auth-login)
    let wt_task_dir = wt.join(".test-mem/auth-login");
    assert!(
        !wt_task_dir.exists(),
        "task directory must not leak into linked worktree"
    );

    // Verify switching to master from worktree updates worktree HEAD
    env.command()
        .current_dir(&wt)
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("master")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to global context\n"));

    let wt_head_content = std::fs::read_to_string(&wt_head)?;
    assert_eq!(wt_head_content.trim(), "master");

    Ok(())
}
