mod helpers;

use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;

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
fn switch_branch_no_match_bails_without_changing_head() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // First, pin HEAD to a known task so we can verify it is NOT clobbered.
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success();

    // No task cards exist, so --branch matches nothing. The command must
    // fail (non-zero exit) and leave HEAD unchanged.
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("--branch")
        .arg("ghost-branch")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no task matched branch: ghost-branch",
        ));

    // HEAD must still point at the previously-set task.
    let head = std::fs::read_to_string(env.root().join(".test-mem/HEAD"))?;
    assert_eq!(head.trim(), "auth-login");

    Ok(())
}

#[test]
fn switch_branch_matches_scalar() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Task card with a scalar branch: field
    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("auth-login.md"),
        "---\ntitle: Login\nbranch: feat/login\n---\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("switch")
            .arg("--branch")
            .arg("feat/login")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;
    assert_eq!(json["context"], "auth-login");

    Ok(())
}

#[test]
fn switch_branch_matches_inline_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("multi.md"),
        "---\nbranch: [feat/one, feat/two]\n---\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("switch")
            .arg("--branch")
            .arg("feat/two")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;
    assert_eq!(json["context"], "multi");

    Ok(())
}

#[test]
fn switch_branch_matches_multiline_block_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("block.md"),
        "---\nbranch:\n  - feat/alpha\n  - feat/beta\n---\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("switch")
            .arg("--branch")
            .arg("feat/beta")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;
    assert_eq!(json["context"], "block");

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

// ---------------------------------------------------------------------------
// Proxy worktree: cue switch respects STORE redirect
//
// Acceptance criterion 7: in a proxy worktree, cue switch must write HEAD to
// the local head_dir and create the scope directory under the STORE target
// (store_dir), not under the local .cue/.
// ---------------------------------------------------------------------------

/// Create a minimal valid cue store (has master/ subdir) at <parent>/.cue.
fn make_real_store(parent: &Path) -> std::path::PathBuf {
    let store = parent.join(".cue");
    fs::create_dir_all(store.join("master")).unwrap();
    store
}

#[test]
fn switch_in_proxy_writes_head_locally_scope_dir_in_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();

    // The shared real store lives outside the worktree's git root.
    let real_store = make_real_store(env.root());

    // The worktree is its own git repo (simulates a real git worktree where
    // `git rev-parse --show-toplevel` returns the worktree path itself).
    let worktree = env.root().join("worktrees/agent1");
    fs::create_dir_all(&worktree)?;
    helpers::setup_git_repo(&worktree);

    // Link the worktree to the shared store (creates proxy .cue/STORE).
    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .assert()
        .success();

    // Switch context inside the proxy worktree.
    env.command()
        .current_dir(&worktree)
        .arg("switch")
        .arg("proj-123-impl")
        .assert()
        .success();

    // HEAD must be written to the LOCAL proxy .cue/HEAD.
    let local_head = worktree.join(".cue/HEAD");
    assert!(
        local_head.exists(),
        "HEAD must be written to local proxy .cue/HEAD"
    );
    assert_eq!(fs::read_to_string(&local_head)?.trim(), "proj-123-impl");

    // The scope directory must be created in the SHARED store (STORE target),
    // not in the worktree's local .cue/.
    let store_scope_dir = real_store.join("proj-123-impl");
    assert!(
        store_scope_dir.is_dir(),
        "scope dir must be created in shared store: {}",
        store_scope_dir.display()
    );

    // No local scope directory leaks into the worktree.
    assert!(
        !worktree.join("proj-123-impl").exists(),
        "scope dir must NOT be created inside the worktree"
    );

    Ok(())
}
