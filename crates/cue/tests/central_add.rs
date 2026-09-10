mod helpers;

use predicates::prelude::*;
use serde_yaml::Value;
use std::path::Path;

/// Parse the YAML frontmatter block of a markdown artifact.
fn read_frontmatter(path: &Path) -> anyhow::Result<Value> {
    let content = std::fs::read_to_string(path)?;
    let frontmatter = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("markdown artifact should contain YAML frontmatter");
    Ok(serde_yaml::from_str(frontmatter)?)
}

/// The short hash of the fixture repository's current revision.
fn head_hash(repo: &Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo)
        .output()?;
    assert!(output.status.success());
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[test]
fn add_creates_a_task_inside_an_explicit_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/task/publish.md");
    let content = std::fs::read_to_string(path)?;
    let (frontmatter, body) = content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("---\n"))
        .expect("task should contain YAML frontmatter");
    let metadata: Value = serde_yaml::from_str(frontmatter)?;

    assert_eq!(metadata["status"], "inbox");
    assert_eq!(metadata["priority"], "normal");
    assert!(metadata["created_at"].as_u64().is_some());
    assert!(metadata.get("kind").is_none());
    assert_eq!(body, "Publish the release");
    assert!(!env.root().join(".cue").exists());

    Ok(())
}

#[test]
fn add_honors_explicit_task_status_and_priority() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
            "--context",
            "release",
            "--frontmatter",
            "status=in-progress",
            "--frontmatter",
            "priority=high",
        ])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/task/publish.md");
    let metadata = read_frontmatter(&path)?;

    assert_eq!(metadata["status"], "in-progress");
    assert_eq!(metadata["priority"], "high");

    Ok(())
}

#[test]
fn add_preserves_conventional_task_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
            "--context",
            "release",
            "--frontmatter",
            "kind=deploy",
            "--frontmatter",
            "owner=release-team",
        ])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/task/publish.md");
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
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .env("CUE_CONTEXT", "release")
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join("acme/widgets/release/task/publish.md")
            .is_file()
    );

    Ok(())
}

#[test]
fn add_ignores_legacy_task_environment_variable() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .env("CUE_TASK", "release")
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --context <context>",
        ));
}

#[test]
fn add_uses_context_from_branch_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    let config = std::process::Command::new("git")
        .args(["config", "branch.main.cue-context", "release"])
        .current_dir(env.root())
        .output()?;
    assert!(config.status.success());

    env.command()
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join("acme/widgets/release/task/publish.md")
            .is_file()
    );

    Ok(())
}

#[test]
fn add_ignores_legacy_task_branch_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --context <context>",
        ));

    Ok(())
}

#[test]
fn environment_context_overrides_branch_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for context in ["release", "hotfix"] {
        env.command()
            .args(["context", "create", context])
            .assert()
            .success();
    }

    let config = std::process::Command::new("git")
        .args(["config", "branch.main.cue-context", "release"])
        .current_dir(env.root())
        .output()?;
    assert!(config.status.success());

    env.command()
        .env("CUE_CONTEXT", "hotfix")
        .args(["add", "ship", "Ship the hotfix", "--type", "task"])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join("acme/widgets/hotfix/task/ship.md")
            .is_file()
    );
    assert!(
        !env.cue_store()
            .join("acme/widgets/release/task/ship.md")
            .exists()
    );

    Ok(())
}

#[test]
fn add_rejects_write_without_active_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["add", "publish", "Publish the release", "--type", "task"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --context <context>",
        ));
}

#[test]
fn add_creates_named_spec_with_structured_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
            "--context",
            "release",
            "--frontmatter",
            "audience=operators",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
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
                "--context",
                "release",
            ])
            .assert()
            .success();

        let path = env
            .cue_store()
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
fn add_creates_markdown_artifacts_in_nested_directories() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "ideas/rollout",
            "Rollout idea",
            "--type",
            "note",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/note/ideas/rollout.md");
    assert!(std::fs::read_to_string(path)?.ends_with("Rollout idea"));

    Ok(())
}

#[test]
fn add_stamps_trace_revision_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke-run",
            "Observed output",
            "--type",
            "trace",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/trace/smoke-run.md");
    let metadata = read_frontmatter(&path)?;

    assert_eq!(metadata["repo_id"], "acme/widgets");
    assert_eq!(metadata["commit_hash"], head_hash(env.root())?);

    Ok(())
}

#[test]
fn add_keeps_revision_metadata_off_other_markdown_types() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for cue_type in ["task", "spec", "plan", "note"] {
        env.command()
            .args([
                "add",
                cue_type,
                "Artifact body",
                "--type",
                cue_type,
                "--context",
                "release",
            ])
            .assert()
            .success();

        let path = env
            .cue_store()
            .join("acme/widgets/release")
            .join(cue_type)
            .join(format!("{cue_type}.md"));
        let metadata = read_frontmatter(&path)?;

        assert!(metadata.get("repo_id").is_none(), "{cue_type} got repo_id");
        assert!(
            metadata.get("commit_hash").is_none(),
            "{cue_type} got commit_hash"
        );
    }

    Ok(())
}

#[test]
fn add_honors_explicit_trace_revision_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "upstream-run",
            "Observed output",
            "--type",
            "trace",
            "--context",
            "release",
            "--frontmatter",
            "repo_id=upstream/library",
            "--frontmatter",
            "commit_hash=0badcafe",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/trace/upstream-run.md");
    let metadata = read_frontmatter(&path)?;

    assert_eq!(metadata["repo_id"], "upstream/library");
    assert_eq!(metadata["commit_hash"], "0badcafe");

    Ok(())
}

#[test]
fn add_creates_json_bin_with_top_level_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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

    let path = env
        .cue_store()
        .join("acme/widgets/release/bin/analysis.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;

    assert_eq!(content["findings"][0], "ready");
    assert_eq!(content["analyzer"], "smoke-test");

    Ok(())
}

#[test]
fn add_creates_json_artifacts_in_nested_directories() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "reports/readiness",
            r#"{"ready":true}"#,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/bin/reports/readiness.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    assert_eq!(content["ready"], true);

    Ok(())
}

#[test]
fn add_creates_a_named_tmp_group_for_the_current_revision() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "reports/check.txt",
            "check output",
            "--type",
            "tmp",
            "--context",
            "release",
            "--group",
            "qa",
        ])
        .assert()
        .success();

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let groups = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(groups.len(), 1);
    let group_name = groups[0].file_name();
    let group_name = group_name.to_string_lossy();
    assert!(group_name.ends_with("-qa"));
    assert_eq!(
        std::fs::read_to_string(groups[0].path().join("reports/check.txt"))?,
        "check output"
    );

    Ok(())
}

#[test]
fn add_reuses_a_tmp_group_for_the_same_revision() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for file in ["first.txt", "second.txt"] {
        env.command()
            .args([
                "add",
                file,
                "output",
                "--type",
                "tmp",
                "--context",
                "release",
                "--group",
                "qa",
            ])
            .assert()
            .success();
    }

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let groups = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(groups.len(), 1);
    assert!(groups[0].path().join("first.txt").is_file());
    assert!(groups[0].path().join("second.txt").is_file());

    Ok(())
}

#[test]
fn add_separates_tmp_groups_by_name() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for group in ["qa", "bench"] {
        env.command()
            .args([
                "add",
                "run.txt",
                "output",
                "--type",
                "tmp",
                "--context",
                "release",
                "--group",
                group,
            ])
            .assert()
            .success();
    }

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let mut groups = std::fs::read_dir(&tmp_dir)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    groups.sort();
    assert_eq!(groups.len(), 2);
    assert!(groups[0].ends_with("-bench"));
    assert!(groups[1].ends_with("-qa"));

    Ok(())
}

#[test]
fn add_creates_a_new_tmp_group_for_a_new_revision() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "first.txt",
            "output",
            "--type",
            "tmp",
            "--context",
            "release",
            "--group",
            "qa",
        ])
        .assert()
        .success();

    env.commit_new_revision("change.txt");

    env.command()
        .args([
            "add",
            "second.txt",
            "output",
            "--type",
            "tmp",
            "--context",
            "release",
            "--group",
            "qa",
        ])
        .assert()
        .success();

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let groups = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(groups.len(), 2);
    assert!(
        groups
            .iter()
            .all(|group| group.file_name().to_string_lossy().ends_with("-qa"))
    );
    assert_eq!(
        groups
            .iter()
            .filter(|group| group.path().join("first.txt").is_file())
            .count(),
        1
    );
    assert_eq!(
        groups
            .iter()
            .filter(|group| group.path().join("second.txt").is_file())
            .count(),
        1
    );

    Ok(())
}
