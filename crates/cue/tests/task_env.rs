mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

#[test]
fn task_env_routes_add_to_env_scope() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command().arg("init").assert().success();

    // With CUE_TASK set, `cue add` writes to the env-specified scope without --task
    env.command()
        .env("CUE_TASK", "auth-feature")
        .arg("add")
        .arg("--type")
        .arg("note")
        .arg("--root")
        .arg("my-note.md")
        .arg("Note content")
        .assert()
        .success()
        .stdout(predicate::str::contains(".cue/auth-feature/note/"));

    assert!(env
        .root()
        .join(".cue/auth-feature/note/my-note.md")
        .exists());

    Ok(())
}

#[test]
fn task_flag_overrides_task_env() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command().arg("init").assert().success();

    // With CUE_TASK set AND --task passed, --task wins
    env.command()
        .env("CUE_TASK", "auth-feature")
        .arg("add")
        .arg("--type")
        .arg("note")
        .arg("--root")
        .arg("--task")
        .arg("override-task")
        .arg("flag-note.md")
        .arg("Override content")
        .assert()
        .success()
        .stdout(predicate::str::contains(".cue/override-task/note/"));

    assert!(env
        .root()
        .join(".cue/override-task/note/flag-note.md")
        .exists());
    assert!(!env
        .root()
        .join(".cue/auth-feature/note/flag-note.md")
        .exists());

    Ok(())
}

#[test]
fn task_env_routes_log_and_list() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command().arg("init").assert().success();

    // With CUE_TASK set, log add writes to the task scope
    env.command()
        .env("CUE_TASK", "env-scoped-task")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Env Log Title")
        .assert()
        .success()
        .stdout(predicate::str::contains(".cue/env-scoped-task/log.md"));

    let log_content = fs::read_to_string(env.root().join(".cue/env-scoped-task/log.md"))?;
    assert!(log_content.contains("Env Log Title"));

    // And log list reads from the task scope
    env.command()
        .env("CUE_TASK", "env-scoped-task")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Env Log Title"));

    Ok(())
}

#[test]
fn task_env_empty_falls_back_to_head() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command().arg("init").assert().success();
    env.command()
        .arg("switch")
        .arg("head-scoped-task")
        .assert()
        .success();

    // Empty CUE_TASK should fall back to HEAD (head-scoped-task)
    env.command()
        .env("CUE_TASK", "")
        .arg("add")
        .arg("--type")
        .arg("note")
        .arg("--root")
        .arg("head-note.md")
        .arg("Head content")
        .assert()
        .success()
        .stdout(predicate::str::contains(".cue/head-scoped-task/note/"));

    assert!(env
        .root()
        .join(".cue/head-scoped-task/note/head-note.md")
        .exists());

    Ok(())
}
