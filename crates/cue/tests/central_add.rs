mod helpers;

use predicates::prelude::*;
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

#[test]
fn add_rejects_write_without_active_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    env.command()
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --task <context>",
        ));
}

#[test]
fn add_creates_named_spec_with_structured_metadata() -> anyhow::Result<()> {
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
            "requirements",
            "Release requirements",
            "--type",
            "spec",
            "--task",
            "release",
            "--frontmatter",
            "audience=operators",
        ])
        .assert()
        .success();

    let path = env
        .cue_home()
        .join("acme/widgets/release/spec/requirements.md");
    let content = std::fs::read_to_string(path)?;
    let (frontmatter, body) = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .expect("spec should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["audience"], "operators");
    assert!(metadata["created_at"].as_u64().is_some());
    assert_eq!(body, "Release requirements");

    Ok(())
}

#[test]
fn add_creates_each_named_markdown_artifact_type() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for cue_type in ["plan", "note", "trace"] {
        let name = format!("release-{cue_type}");
        env.command()
            .args([
                "add",
                &name,
                "Artifact body",
                "--type",
                cue_type,
                "--task",
                "release",
            ])
            .assert()
            .success();

        let path = env
            .cue_home()
            .join("acme/widgets/release")
            .join(cue_type)
            .join(format!("{name}.md"));
        let content = std::fs::read_to_string(path)?;
        let frontmatter = content
            .strip_prefix("---\n")
            .and_then(|content| content.split_once("---\n"))
            .map(|(frontmatter, _)| frontmatter)
            .expect("markdown artifact should contain YAML frontmatter");
        let metadata: Value = serde_yaml::from_str(frontmatter)?;
        assert!(metadata["created_at"].as_u64().is_some());
    }

    Ok(())
}

#[test]
fn add_creates_json_bin_with_top_level_metadata() -> anyhow::Result<()> {
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
            "analysis",
            r#"{"findings":["ready"]}"#,
            "--type",
            "bin",
            "--task",
            "release",
            "--frontmatter",
            "analyzer=smoke-test",
        ])
        .assert()
        .success();

    let path = env
        .cue_home()
        .join("acme/widgets/release/bin/analysis.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;

    assert_eq!(content["findings"][0], "ready");
    assert_eq!(content["analyzer"], "smoke-test");

    Ok(())
}
