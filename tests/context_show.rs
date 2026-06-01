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

    let context_json = env.root().join(".mem").join("main").join("context.json");
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

    env.command()
        .arg("context")
        .arg("show")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Context file not found"));

    Ok(())
}
