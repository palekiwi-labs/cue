mod helpers;

use serde_yaml::Value;

#[test]
fn add_creates_a_task_inside_an_explicit_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "publish",
            "Publish the release",
            "--type",
            "task",
            "--task",
            "release",
        ])
        .assert()
        .success();

    let path = env.cue_home().join("acme/widgets/release/task/publish.md");
    let content = std::fs::read_to_string(path)?;
    let (frontmatter, body) = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .expect("task should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["status"], "inbox");
    assert!(metadata["created_at"].as_u64().is_some());
    assert!(metadata.get("kind").is_none());
    assert_eq!(body, "Publish the release");
    assert!(!env.root().join(".cue").exists());

    Ok(())
}

#[test]
fn add_preserves_conventional_task_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "publish",
            "Publish the release",
            "--type",
            "task",
            "--task",
            "release",
            "--frontmatter",
            "kind=deploy",
            "--frontmatter",
            "owner=release-team",
        ])
        .assert()
        .success();

    let path = env.cue_home().join("acme/widgets/release/task/publish.md");
    let content = std::fs::read_to_string(path)?;
    let frontmatter = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("task should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["kind"], "deploy");
    assert_eq!(metadata["owner"], "release-team");

    Ok(())
}

#[test]
fn add_uses_context_from_environment() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .env("CUE_TASK", "release")
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_home()
            .join("acme/widgets/release/task/publish.md")
            .is_file()
    );

    Ok(())
}

#[test]
fn add_uses_context_from_branch_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    let config = std::process::Command::new("git")
        .args(["config", "branch.main.cue-task", "release"])
        .current_dir(env.root())
        .output()?;
    assert!(config.status.success());

    env.command()
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_home()
            .join("acme/widgets/release/task/publish.md")
            .is_file()
    );

    Ok(())
}

#[test]
fn environment_context_overrides_branch_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    for context in ["release", "hotfix"] {
        env.command()
            .args(["context", "create", context])
            .assert()
            .success();
    }

    let config = std::process::Command::new("git")
        .args(["config", "branch.main.cue-task", "release"])
        .current_dir(env.root())
        .output()?;
    assert!(config.status.success());

    env.command()
        .env("CUE_TASK", "hotfix")
        .args(["add", "ship", "Ship the hotfix", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_home()
            .join("acme/widgets/hotfix/task/ship.md")
            .is_file()
    );
    assert!(
        !env.cue_home()
            .join("acme/widgets/release/task/ship.md")
            .exists()
    );

    Ok(())
}
