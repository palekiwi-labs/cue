mod helpers;

use predicates::prelude::*;
use std::fs;
use std::path::Path;

/// Create a minimal valid cue store (has master/ subdir).
fn make_real_store(parent: &Path) -> std::path::PathBuf {
    let store = parent.join(".cue");
    fs::create_dir_all(store.join("master")).unwrap();
    store
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[test]
fn link_creates_proxy_cue_with_store_file() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent1");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .assert()
        .success();

    let store_file = worktree.join(".cue/STORE");
    assert!(store_file.exists(), ".cue/STORE must be created");

    let content = fs::read_to_string(&store_file)?;
    assert_eq!(content.trim(), real_store.canonicalize()?.to_str().unwrap());

    // Without --task, HEAD must NOT be written.
    assert!(
        !worktree.join(".cue/HEAD").exists(),
        ".cue/HEAD must not be written when --task is omitted"
    );

    Ok(())
}

#[test]
fn link_with_task_writes_head() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent2");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .arg("--task")
        .arg("proj-123-impl")
        .assert()
        .success();

    let head = fs::read_to_string(worktree.join(".cue/HEAD"))?;
    assert_eq!(head.trim(), "proj-123-impl");

    Ok(())
}

#[test]
fn link_with_task_master_is_permitted() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/orch");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .arg("--task")
        .arg("master")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());

    let head = fs::read_to_string(worktree.join(".cue/HEAD"))?;
    assert_eq!(head.trim(), "master");

    Ok(())
}

// ---------------------------------------------------------------------------
// Task card missing: warn but still exit 0
// ---------------------------------------------------------------------------

#[test]
fn link_task_without_card_warns_on_stderr_exits_zero() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent3");
    fs::create_dir_all(&worktree)?;

    // no task card at <store>/master/task/no-card.md
    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .arg("--task")
        .arg("no-card")
        .assert()
        .success()
        .stderr(predicate::str::contains("no-card"));

    Ok(())
}

#[test]
fn link_task_with_matching_card_no_stderr_warning() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    // Create the matching task card.
    let task_dir = real_store.join("master/task");
    fs::create_dir_all(&task_dir)?;
    fs::write(task_dir.join("proj-123.md"), "---\ntitle: Test\n---\n")?;

    let worktree = env.root().join("worktrees/agent4");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .arg("--task")
        .arg("proj-123")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());

    Ok(())
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn link_store_path_not_exists_errors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();

    let worktree = env.root().join("worktrees/agent5");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg("/nonexistent/path/.cue")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));

    Ok(())
}

#[test]
fn link_store_missing_master_subdir_errors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();

    // A directory that exists but has no master/ inside.
    let invalid_store = env.root().join("invalid-store");
    fs::create_dir_all(&invalid_store)?;

    let worktree = env.root().join("worktrees/agent6");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(invalid_store.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("master/"));

    Ok(())
}

#[test]
fn link_cue_already_exists_errors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent7");
    fs::create_dir_all(&worktree)?;

    // Pre-create a .cue/ dir.
    fs::create_dir_all(worktree.join(".cue"))?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains(".cue"));

    Ok(())
}

#[test]
fn link_traversal_slug_is_rejected() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent8");
    fs::create_dir_all(&worktree)?;

    env.command()
        .current_dir(&worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .arg("--task")
        .arg("../../evil")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid task slug"));

    Ok(())
}

// ---------------------------------------------------------------------------
// --dir flag (global -C) targets a different directory
// ---------------------------------------------------------------------------

#[test]
fn link_with_dir_flag_creates_proxy_in_target() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store(env.root());

    let worktree = env.root().join("worktrees/agent9");
    fs::create_dir_all(&worktree)?;

    // Run from root but point --dir at the worktree.
    env.command()
        .arg("-C")
        .arg(worktree.to_str().unwrap())
        .arg("link")
        .arg(real_store.to_str().unwrap())
        .assert()
        .success();

    assert!(worktree.join(".cue/STORE").exists());

    Ok(())
}
