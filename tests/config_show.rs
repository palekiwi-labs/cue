mod helpers;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_show_json() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Create a project-specific config
    fs::write(
        temp.path().join("mem.json"),
        r#"{"dir_name": ".custom-mem"}"#,
    )?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-branch")
        .arg("config")
        .arg("show");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""dir_name": ".custom-mem""#))
        .stdout(predicate::str::contains(r#""branch_name": "test-branch""#));

    Ok(())
}
