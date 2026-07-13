mod helpers;

use predicates::prelude::*;
use serde_json::Value;

// ── status --json ────────────────────────────────────────────────────────────

#[test]
fn status_json_global_when_head_absent() -> anyhow::Result<()> {
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
            .arg("status")
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
    assert!(json.get("title").is_none());
    assert!(json.get("status").is_none());

    Ok(())
}

#[test]
fn status_json_task_with_card() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Switch to a task so HEAD is populated
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success();

    // Write a task card with title and status frontmatter
    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("auth-login.md"),
        "---\ntitle: Implement Login\nstatus: in-progress\n---\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("status")
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
    assert_eq!(json["title"], "Implement Login");
    assert_eq!(json["status"], "in-progress");

    Ok(())
}

#[test]
fn status_json_task_with_crlf_frontmatter() -> anyhow::Result<()> {
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
        .success();

    // Task card with CRLF line endings — the old hand-rolled parser failed
    // on this because it hard-coded "---\n".
    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("auth-login.md"),
        "---\r\ntitle: CRLF Title\r\nstatus: open\r\n---\r\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("status")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: Value = serde_json::from_str(output.trim())?;

    assert_eq!(json["context"], "auth-login");
    assert_eq!(json["title"], "CRLF Title");
    assert_eq!(json["status"], "open");

    Ok(())
}

#[test]
fn status_human_output_global() -> anyhow::Result<()> {
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
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::diff("active context: master (global)\n"));

    Ok(())
}

#[test]
fn status_human_output_task_with_card() -> anyhow::Result<()> {
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
        .success();

    let task_dir = env.root().join(".test-mem/master/task");
    std::fs::create_dir_all(&task_dir)?;
    std::fs::write(
        task_dir.join("auth-login.md"),
        "---\ntitle: Implement Login\nstatus: in-progress\n---\n# Body",
    )?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("active task: auth-login"))
        .stdout(predicate::str::contains("title: Implement Login"))
        .stdout(predicate::str::contains("status: in-progress"))
        .stdout(predicate::str::contains("context: .cue/auth-login/"));

    Ok(())
}
