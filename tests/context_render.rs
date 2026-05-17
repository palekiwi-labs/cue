mod helpers;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_context_render_with_globs() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path()).arg("init").assert().success();

    // Create some spec files
    let spec_dir = temp.path().join(".mem").join("main").join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("1.md"), "content 1")?;
    fs::write(spec_dir.join("2.md"), "content 2")?;

    // Create context.json with glob
    let context_json = temp.path().join(".mem").join("main").join("context.json");
    fs::write(
        &context_json,
        r#"{
        "default": {
            "artifacts": ["./spec/*.md"]
        }
    }"#,
    )?;

    // Run mem context render
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
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
