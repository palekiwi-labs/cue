mod helpers;

use predicates::prelude::*;

#[test]
fn test_project_add_registers_cwd() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .args(["project", "add"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Registered"));

    // Verify the store file was created
    assert!(env.data_dir.join("projects.json").exists());

    Ok(())
}

#[test]
fn test_project_add_with_explicit_path() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .args(["project", "add", "--path", env.root().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Registered"));

    Ok(())
}

#[test]
fn test_project_add_is_idempotent() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // First add
    env.command().args(["project", "add"]).assert().success();

    // Second add — no error, no duplicate entry
    env.command()
        .args(["project", "add"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("already registered")
                .or(predicate::str::contains("Registered")),
        );

    // Read the JSON and confirm there's only one path for the key
    let content = std::fs::read_to_string(env.data_dir.join("projects.json"))?;
    let count = content.matches(env.root().to_str().unwrap()).count();
    assert_eq!(count, 1, "path should appear exactly once in store");

    Ok(())
}

#[test]
fn test_project_list_shows_registered_projects() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command().args(["project", "add"]).assert().success();

    env.command()
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(env.root().to_str().unwrap()));

    Ok(())
}

#[test]
fn test_project_list_empty_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_project_remove_by_path() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Add first
    env.command().args(["project", "add"]).assert().success();

    // Remove by path
    env.command()
        .args(["project", "remove"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    // List should be empty now
    env.command()
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_project_remove_by_key() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Add first
    env.command().args(["project", "add"]).assert().success();

    // Determine the key from the list output
    let list_out = env.command().args(["project", "list"]).output()?;
    let list_str = String::from_utf8_lossy(&list_out.stdout);
    // The list output is "key  path" lines; grab the first word
    let key = list_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .expect("expected at least one listed project");

    // Remove by key
    env.command()
        .args(["project", "remove", "--key", key])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    Ok(())
}
