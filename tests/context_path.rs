mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_context_path_current() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    let context_json = env.root().join(".mem").join("main").join("context.json");
    fs::create_dir_all(context_json.parent().unwrap())?;
    fs::write(&context_json, "{}")?;

    // Test path
    env.command()
        .arg("context")
        .arg("path")
        .assert()
        .success()
        .stdout(predicate::str::contains(context_json.to_str().unwrap()));

    Ok(())
}

#[test]
fn test_context_path_all() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    let context_main = env.root().join(".mem").join("main").join("context.json");
    let context_feat = env.root().join(".mem").join("feat").join("context.json");

    fs::create_dir_all(context_main.parent().unwrap())?;
    fs::create_dir_all(context_feat.parent().unwrap())?;
    fs::write(&context_main, "{}")?;
    fs::write(&context_feat, "{}")?;

    // Test path --all
    let assert = env
        .command()
        .arg("context")
        .arg("path")
        .arg("--all")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())?;
    assert!(stdout.contains(context_main.to_str().unwrap()));
    assert!(stdout.contains(context_feat.to_str().unwrap()));

    Ok(())
}

#[test]
fn test_context_path_missing_errors() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .arg("context")
        .arg("path")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Context file not found for branch: main",
        ));

    Ok(())
}
