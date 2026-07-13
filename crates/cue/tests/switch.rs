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
