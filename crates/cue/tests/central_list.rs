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
            "--context",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args(["list", "--context", "release"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            env.cue_store()
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
                "add",
                artifact,
                "Notes",
                "--type",
                "note",
                "--context",
                context,
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
fn list_only_returns_supported_artifact_types() -> anyhow::Result<()> {
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
            "--context",
            "release",
        ])
        .assert()
        .success();
    env.command()
        .args([
            "log",
            "add",
            "--context",
            "release",
            "--title",
            "Validated release",
        ])
        .assert()
        .success();

    let unsupported_dir = env.cue_store().join("acme/widgets/release/attachments");
    std::fs::create_dir_all(&unsupported_dir)?;
    std::fs::write(unsupported_dir.join("report.txt"), "not an artifact")?;

    let output = env
        .command()
        .args(["list", "--context", "release", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["type"], "note");
    assert_eq!(artifacts[0]["name"], "decisions.md");

    Ok(())
}

#[test]
fn list_includes_tmp_as_a_supported_artifact_type() -> anyhow::Result<()> {
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
            "report.txt",
            "temporary report",
            "--type",
            "tmp",
            "--context",
            "release",
            "--group",
            "qa",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["list", "--context", "release", "--type", "tmp", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["type"], "tmp");
    assert!(
        artifacts[0]["name"]
            .as_str()
            .unwrap()
            .ends_with("/report.txt")
    );

    Ok(())
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
            "--context",
            "release",
            "--frontmatter",
            "audience=operators",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["list", "--context", "release", "--frontmatter"])
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
            "--context",
            "release",
            "--frontmatter",
            "analyzer=smoke-test",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["list", "--context", "release", "--frontmatter"])
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

#[test]
fn list_uses_the_branch_configured_active_context() -> anyhow::Result<()> {
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
                "add",
                artifact,
                "Notes",
                "--type",
                "note",
                "--context",
                context,
            ])
            .assert()
            .success();
    }

    let config = std::process::Command::new("git")
        .args(["config", "branch.main.cue-context", "release"])
        .current_dir(env.root())
        .output()?;
    assert!(config.status.success());

    env.command()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("release/note/decisions.md"))
        .stdout(predicate::str::contains("hotfix/note/incident.md").not());

    Ok(())
}
