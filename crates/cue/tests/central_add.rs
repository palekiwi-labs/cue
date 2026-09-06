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
