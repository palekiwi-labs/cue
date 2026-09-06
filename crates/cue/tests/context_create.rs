mod helpers;

use serde_yaml::Value;

#[test]
fn context_create_writes_a_context_record_to_the_central_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    env.command()
        .args(["context", "create", "data-model-spike"])
        .assert()
        .success()
        .stdout("Created acme/widgets/data-model-spike\n");

    let path = env
        .cue_home()
        .join("acme/widgets/data-model-spike/context.md");
    let content = std::fs::read_to_string(path)?;
    let frontmatter = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("context.md should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["kind"], "work");
    assert!(metadata["created_at"].as_u64().is_some());
    assert_eq!(metadata.as_mapping().map(|fields| fields.len()), Some(2));
    assert!(!env.root().join(".cue").exists());

    Ok(())
}

#[test]
fn context_create_accepts_a_context_kind() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    env.command()
        .args([
            "context",
            "create",
            "architecture-notes",
            "--kind",
            "reference",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(
        env.cue_home()
            .join("acme/widgets/architecture-notes/context.md"),
    )?;
    assert!(content.contains("kind: reference\n"));

    Ok(())
}
