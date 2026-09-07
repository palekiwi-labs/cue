mod helpers;

use predicates::prelude::*;

#[test]
fn list_reads_artifacts_from_an_explicit_central_context() {
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
            "decisions",
            "Release decisions",
            "--type",
            "note",
            "--task",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args(["list", "--task", "release"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            env.cue_home()
                .join("acme/widgets/release/note/decisions.md")
                .to_string_lossy()
                .as_ref(),
        ));
}

#[test]
fn list_without_an_active_context_reads_the_repository_scope() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    for (context, artifact) in [("release", "decisions"), ("hotfix", "incident")] {
        env.command()
            .args(["context", "create", context])
            .assert()
            .success();
        env.command()
            .args([
                "add", artifact, "Notes", "--type", "note", "--task", context,
            ])
            .assert()
            .success();
    }

    env.command()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("release/note/decisions.md"))
        .stdout(predicate::str::contains("hotfix/note/incident.md"));
}

#[test]
fn list_emits_central_artifact_metadata_as_json() -> anyhow::Result<()> {
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
            "decisions",
            "Release decisions",
            "--type",
            "note",
            "--task",
            "release",
            "--frontmatter",
            "audience=operators",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["list", "--task", "release", "--frontmatter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;
    let artifact = &artifacts[0];

    assert_eq!(artifact["context"], "release");
    assert_eq!(artifact["type"], "note");
    assert_eq!(artifact["name"], "decisions.md");
    assert_eq!(artifact["frontmatter"]["audience"], "operators");
    assert!(artifact.get("branch").is_none());
    assert!(artifact.get("commit_timestamp").is_none());

    Ok(())
}

#[test]
fn list_reads_top_level_json_artifact_metadata() -> anyhow::Result<()> {
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

    let output = env
        .command()
        .args(["list", "--task", "release", "--frontmatter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;
    let artifact = &artifacts[0];

    assert_eq!(artifact["type"], "bin");
    assert_eq!(artifact["frontmatter"]["analyzer"], "smoke-test");
    assert_eq!(artifact["frontmatter"]["findings"][0], "ready");

    Ok(())
}
