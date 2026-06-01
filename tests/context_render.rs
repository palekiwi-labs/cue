mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_context_render_with_globs() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    // Create some spec files
    let spec_dir = env.root().join(".mem").join("main").join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("1.md"), "content 1")?;
    fs::write(spec_dir.join("2.md"), "content 2")?;

    // Create context.json with glob
    let context_json = env.root().join(".mem").join("main").join("context.json");
    fs::write(
        &context_json,
        r#"{
        "default": {
            "artifacts": ["./spec/*.md"]
        }
    }"#,
    )?;

    // Run mem context render
    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("content 1"))
        .stdout(predicate::str::contains("content 2"))
        .stdout(predicate::str::contains("path=\".mem/main/spec/1.md\""))
        .stdout(predicate::str::contains("path=\".mem/main/spec/2.md\""));

    Ok(())
}

#[test]
fn test_context_render_instructions() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command().arg("init").assert().success();

    // Create context.json with instructions
    let mem_main = env.root().join(".mem").join("main");
    fs::create_dir_all(&mem_main)?;
    let context_json = mem_main.join("context.json");
    fs::write(
        &context_json,
        r#"{
        "default": {
            "artifacts": [],
            "instructions": "Please implement the feature"
        }
    }"#,
    )?;

    // Run mem context render
    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "<instructions>\nPlease implement the feature\n</instructions>",
        ));

    Ok(())
}
