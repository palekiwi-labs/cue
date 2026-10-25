mod helpers;

use predicates::prelude::*;

#[test]
fn list_reads_artifacts_from_an_explicit_central_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "readiness",
            r#"{"findings":["ready"]}"#,
            "--type",
            "review",
            "--context",
            "release",
            "--frontmatter",
            "reviewer=smoke-test",
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

    assert_eq!(artifact["type"], "review");
    assert_eq!(artifact["frontmatter"]["reviewer"], "smoke-test");
    assert_eq!(artifact["frontmatter"]["findings"][0], "ready");

    Ok(())
}

#[test]
fn list_reads_nested_json_artifact_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "rounds/first",
            r#"{"findings":["ready"]}"#,
            "--type",
            "review",
            "--context",
            "release",
            "--frontmatter",
            "verdict=approve",
        ])
        .assert()
        .success();

    // The artifact type is the path segment below the context, not the
    // directory the file happens to sit in, so grouping a review into a
    // subdirectory must not demote it to frontmatter parsing.
    let output = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--filter",
            "verdict=approve",
            "--frontmatter",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["type"], "review");
    assert_eq!(artifacts[0]["name"], "rounds/first.json");
    assert_eq!(artifacts[0]["frontmatter"]["findings"][0], "ready");

    Ok(())
}

#[test]
fn list_reads_markdown_metadata_inside_a_type_named_directory() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for filename in ["review/decisions", "bin/decisions"] {
        env.command()
            .args([
                "add",
                filename,
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
    }

    // A note grouped under a directory named after another artifact type is
    // still a note: its metadata is frontmatter, and it stays filterable.
    let output = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--filter",
            "audience=operators",
            "--frontmatter",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(2));
    for artifact in artifacts.as_array().unwrap() {
        assert_eq!(artifact["type"], "note");
        assert_eq!(artifact["frontmatter"]["audience"], "operators");
    }
    assert_eq!(artifacts[0]["name"], "bin/decisions.md");
    assert_eq!(artifacts[1]["name"], "review/decisions.md");

    Ok(())
}

#[test]
fn list_skips_metadata_for_opaque_artifact_content() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for (cue_type, filename) in [
        ("bin", "analysis"),
        ("review", "readiness"),
        ("tmp", "report.json"),
    ] {
        env.command()
            .args([
                "add",
                filename,
                r#"{"verdict":"approve"}"#,
                "--type",
                cue_type,
                "--context",
                "release",
            ])
            .assert()
            .success();
    }

    // `bin` and `tmp` hold opaque content: cue stores the bytes and decodes
    // no metadata out of them, even when those bytes happen to be JSON.
    let output = env
        .command()
        .args(["list", "--context", "release", "--frontmatter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(3));
    assert_eq!(artifacts[0]["type"], "bin");
    assert!(artifacts[0].get("frontmatter").is_none());
    assert_eq!(artifacts[1]["type"], "review");
    assert_eq!(artifacts[1]["frontmatter"]["verdict"], "approve");
    assert_eq!(artifacts[2]["type"], "tmp");
    assert!(artifacts[2].get("frontmatter").is_none());

    // Having no metadata, opaque artifacts answer no metadata query.
    let output = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--filter",
            "verdict=approve",
            "--frontmatter",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["type"], "review");

    Ok(())
}

#[test]
fn list_includes_review_artifacts_and_filters_by_type() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
            "add",
            "readiness",
            r#"{"findings":[{"severity":"blocker"}]}"#,
            "--type",
            "review",
            "--context",
            "release",
            "--frontmatter",
            "reviewer=smoke-test",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["list", "--context", "release", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(artifacts.as_array().map(Vec::len), Some(2));

    let output = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--type",
            "review",
            "--frontmatter",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["type"], "review");
    assert_eq!(artifacts[0]["name"], "readiness.json");
    assert_eq!(artifacts[0]["frontmatter"]["reviewer"], "smoke-test");
    assert_eq!(
        artifacts[0]["frontmatter"]["findings"][0]["severity"],
        "blocker"
    );

    Ok(())
}

#[test]
fn list_uses_the_branch_configured_active_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

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

#[test]
fn list_matches_any_repeated_artifact_type() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for (name, artifact_type) in [("steps", "plan"), ("ship", "task"), ("ideas", "note")] {
        env.command()
            .args([
                "add",
                name,
                "body",
                "--type",
                artifact_type,
                "--context",
                "release",
            ])
            .assert()
            .success();
    }

    let output = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--type",
            "plan",
            "--type",
            "task",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let artifacts: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(artifacts.as_array().map(Vec::len), Some(2));
    assert_eq!(artifacts[0]["type"], "plan");
    assert_eq!(artifacts[1]["type"], "task");

    Ok(())
}
