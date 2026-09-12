mod helpers;

use predicates::prelude::*;
use serde_yaml::Value;

#[test]
fn context_create_writes_a_context_record_to_the_central_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(!env.cue_store().join("acme/widgets").exists());

    env.command()
        .args(["context", "create", "data-model-spike"])
        .assert()
        .success()
        .stdout("Created acme/widgets/data-model-spike\n");

    let path = env
        .cue_store()
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
fn context_create_honors_global_store_flag() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let flag_store = env.root().join("flag-home");

    env.command()
        .args(["--store"])
        .arg(&flag_store)
        .args(["context", "create", "spike"])
        .assert()
        .success();

    assert!(flag_store.join("acme/widgets/spike/context.md").is_file());
    assert!(
        !env.cue_store().join("acme").exists(),
        "--store must override $CUE_STORE for context creation"
    );

    Ok(())
}

#[test]
fn context_create_accepts_a_context_kind() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

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
        env.cue_store()
            .join("acme/widgets/architecture-notes/context.md"),
    )?;
    assert!(content.contains("kind: reference\n"));

    Ok(())
}

#[test]
fn context_create_accepts_an_advisory_mode() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "create", "implementation", "--mode", "build"])
        .assert()
        .success();

    let content = std::fs::read_to_string(
        env.cue_store()
            .join("acme/widgets/implementation/context.md"),
    )?;
    assert!(content.contains("mode: build\n"));

    Ok(())
}

#[test]
fn context_create_accepts_presentation_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args([
            "context",
            "create",
            "delivery",
            "--title",
            "Release delivery",
            "--description",
            "Coordinate the release",
        ])
        .assert()
        .success();

    let content =
        std::fs::read_to_string(env.cue_store().join("acme/widgets/delivery/context.md"))?;
    assert!(content.contains("title: Release delivery\n"));
    assert!(content.contains("description: Coordinate the release\n"));

    Ok(())
}

#[test]
fn context_create_accepts_relationship_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["parent", "reference"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    // A reference may cross into another repository scope, which has no
    // `cue context create` path from this checkout.
    let foreign = env.cue_store().join("other/project/context");
    std::fs::create_dir_all(&foreign)?;
    std::fs::write(foreign.join("context.md"), "---\nkind: work\n---\n")?;

    env.command()
        .args([
            "context",
            "create",
            "child",
            "--parent",
            "acme/widgets/parent",
            "--ref",
            "acme/widgets/reference/spec/index",
            "--ref",
            "other/project/context",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(env.cue_store().join("acme/widgets/child/context.md"))?;
    let frontmatter = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("context.md should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["parent"], "acme/widgets/parent");
    assert_eq!(
        metadata["refs"],
        Value::Sequence(vec![
            Value::String("acme/widgets/reference/spec/index".into()),
            Value::String("other/project/context".into()),
        ])
    );

    Ok(())
}

#[test]
fn context_create_rejects_non_canonical_relationship_metadata() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "parent"])
        .assert()
        .success();

    for invalid in [
        "/home/operator/cue/acme/widgets/parent",
        "~/cue/acme/widgets/parent",
        "parent",
        "acme/widgets/unknown",
    ] {
        env.command()
            .args(["context", "create", "child", "--parent", invalid])
            .assert()
            .failure()
            .stderr(predicate::str::contains("<org>/<repo>/<context>"));
        env.command()
            .args([
                "context",
                "create",
                "child",
                "--parent",
                "acme/widgets/parent",
                "--ref",
                invalid,
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("<org>/<repo>/<context>"));
    }

    assert!(!env.cue_store().join("acme/widgets/child").exists());
}
