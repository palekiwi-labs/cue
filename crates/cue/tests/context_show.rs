mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_context_show_and_profiles() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    let context_json = env.root().join(".cue").join("master").join("context.json");
    fs::create_dir_all(context_json.parent().unwrap())?;
    fs::write(
        &context_json,
        r#"{
        "default": { "artifacts": ["./spec/index.md"] },
        "brief": { "artifacts": [] }
    }"#,
    )?;

    // Test show
    env.command()
        .arg("context")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""artifacts": ["#))
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("brief"));

    // Test profiles
    env.command()
        .arg("context")
        .arg("profiles")
        .assert()
        .success()
        .stdout("brief\ndefault\n");

    Ok(())
}

#[test]
fn test_context_missing_file_errors() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize cue
    env.command().arg("init").assert().success();

    env.command()
        .arg("context")
        .arg("show")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Context file not found"));

    Ok(())
}

#[test]
fn test_context_show_with_task_flag_and_env() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());
    env.command().arg("init").assert().success();

    let cue_dir = env.root().join(".cue");

    // Create env-task context.json
    let env_task_dir = cue_dir.join("env-task");
    fs::create_dir_all(&env_task_dir)?;
    fs::write(
        env_task_dir.join("context.json"),
        r#"{"default": {"artifacts": ["./spec/env-task.md"]}}"#,
    )?;

    // Create flag-task context.json
    let flag_task_dir = cue_dir.join("flag-task");
    fs::create_dir_all(&flag_task_dir)?;
    fs::write(
        flag_task_dir.join("context.json"),
        r#"{"default": {"artifacts": ["./spec/flag-task.md"]}}"#,
    )?;

    // 1. Env scope
    env.command()
        .env("CUE_TASK", "env-task")
        .arg("context")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("env-task.md"));

    // 2. Flag overrides Env
    env.command()
        .env("CUE_TASK", "env-task")
        .arg("context")
        .arg("show")
        .arg("--task")
        .arg("flag-task")
        .assert()
        .success()
        .stdout(predicate::str::contains("flag-task.md"));

    Ok(())
}
