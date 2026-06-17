mod helpers;

use predicates::prelude::*;
use tempfile::TempDir;

fn cue_cmd_with_data_dir(data_dir: &TempDir) -> assert_cmd::Command {
    let mut cmd = helpers::cue_cmd();
    cmd.env("CUE_DATA_DIR", data_dir.path());
    cmd
}

#[test]
fn test_project_add_registers_cwd() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Registered"));

    // Verify the store file was created
    assert!(data.path().join("projects.json").exists());

    Ok(())
}

#[test]
fn test_project_add_with_explicit_path() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add", "--path", temp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Registered"));

    Ok(())
}

#[test]
fn test_project_add_is_idempotent() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // First add
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success();

    // Second add — no error, no duplicate entry
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("already registered")
                .or(predicate::str::contains("Registered")),
        );

    // Read the JSON and confirm there's only one path for the key
    let content = std::fs::read_to_string(data.path().join("projects.json"))?;
    let count = content.matches(temp.path().to_str().unwrap()).count();
    assert_eq!(count, 1, "path should appear exactly once in store");

    Ok(())
}

#[test]
fn test_project_list_shows_registered_projects() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success();

    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(temp.path().to_str().unwrap()));

    Ok(())
}

#[test]
fn test_project_list_empty_store() -> anyhow::Result<()> {
    let data = TempDir::new()?;
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_project_remove_by_path() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Add first
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success();

    // Remove by path
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "remove"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    // List should be empty now
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_project_remove_by_key() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let data = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Add first
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "add"])
        .assert()
        .success();

    // Determine the key from the list output
    let list_out = cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "list"])
        .output()?;
    let list_str = String::from_utf8_lossy(&list_out.stdout);
    // The list output is "key  path" lines; grab the first word
    let key = list_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .expect("expected at least one listed project");

    // Remove by key
    cue_cmd_with_data_dir(&data)
        .current_dir(temp.path())
        .args(["project", "remove", "--key", key])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    Ok(())
}
