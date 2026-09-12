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
fn add_creates_a_revision_correlated_tmp_directory() -> anyhow::Result<()> {
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
        ])
        .assert()
        .success();

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let directories = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(directories.len(), 1);
    let directory_name = directories[0].file_name();
    let directory_name = directory_name.to_string_lossy();
    let timestamp = directory_name
        .strip_suffix(&format!("-{}", head_hash(env.root())?))
        .expect("tmp directory should end with the current revision");
    timestamp
        .parse::<u128>()
        .expect("tmp directory should start with a timestamp");
    assert_eq!(
        std::fs::read_to_string(directories[0].path().join("reports/check.txt"))?,
        "check output"
    );

    Ok(())
}

#[test]
fn each_tmp_add_creates_a_fresh_directory() -> anyhow::Result<()> {
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
            ])
            .assert()
            .success();
    }

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let directories = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(directories.len(), 2);
    assert_eq!(
        directories
            .iter()
            .filter(|directory| directory.path().join("first.txt").is_file())
            .count(),
        1
    );
    assert_eq!(
        directories
            .iter()
            .filter(|directory| directory.path().join("second.txt").is_file())
            .count(),
        1
    );

    Ok(())
}

#[test]
fn add_rejects_removed_group_argument() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

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
            "qa",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--group'"));
}

#[test]
fn add_correlates_each_tmp_directory_with_its_revision() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let first_hash = head_hash(env.root())?;
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
        ])
        .assert()
        .success();

    env.commit_new_revision("change.txt");

    let second_hash = head_hash(env.root())?;
    env.command()
        .args([
            "add",
            "second.txt",
            "output",
            "--type",
            "tmp",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let tmp_dir = env.cue_store().join("acme/widgets/release/tmp");
    let directories = std::fs::read_dir(&tmp_dir)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(directories.len(), 2);
    let first_dir = directories
        .iter()
        .find(|directory| directory.path().join("first.txt").is_file())
        .expect("first tmp directory");
    let second_dir = directories
        .iter()
        .find(|directory| directory.path().join("second.txt").is_file())
        .expect("second tmp directory");
    assert!(
        first_dir
            .file_name()
            .to_string_lossy()
            .ends_with(&format!("-{first_hash}"))
    );
    assert!(
        second_dir
            .file_name()
            .to_string_lossy()
            .ends_with(&format!("-{second_hash}"))
    );

    Ok(())
}

#[test]
fn add_serializes_a_single_ref_as_a_list() -> anyhow::Result<()> {
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
            "refs=acme/widgets/release/task/publish.md",
        ])
        .assert()
        .success();

    let metadata = read_frontmatter(
        &env.cue_store()
            .join("acme/widgets/release/spec/requirements.md"),
    )?;
    assert_eq!(
        metadata["refs"].as_sequence().map(Vec::as_slice),
        Some(&[Value::String("acme/widgets/release/task/publish.md".into())][..]),
        "a single ref must still serialize as a YAML list"
    );

    Ok(())
}

#[test]
fn add_serializes_repeated_refs_as_a_list() -> anyhow::Result<()> {
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
            "refs=acme/widgets/release/task/publish.md",
            "--frontmatter",
            "refs=acme/widgets/release/note/ideas/rollout.md",
        ])
        .assert()
        .success();

    let metadata = read_frontmatter(
        &env.cue_store()
            .join("acme/widgets/release/spec/requirements.md"),
    )?;
    assert_eq!(
        metadata["refs"].as_sequence().map(Vec::as_slice),
        Some(
            &[
                Value::String("acme/widgets/release/task/publish.md".into()),
                Value::String("acme/widgets/release/note/ideas/rollout.md".into()),
            ][..]
        )
    );

    Ok(())
}

#[test]
fn add_serializes_parent_as_a_scalar() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "rollout",
            "Rollout plan",
            "--type",
            "plan",
            "--context",
            "release",
            "--frontmatter",
            "parent=acme/widgets/release/task/publish.md",
        ])
        .assert()
        .success();

    let metadata = read_frontmatter(&env.cue_store().join("acme/widgets/release/plan/rollout.md"))?;
    assert_eq!(metadata["parent"], "acme/widgets/release/task/publish.md");

    Ok(())
}

#[test]
fn add_rejects_repeated_parent() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "rollout",
            "Rollout plan",
            "--type",
            "plan",
            "--context",
            "release",
            "--frontmatter",
            "parent=acme/widgets/release/task/publish.md",
            "--frontmatter",
            "parent=acme/widgets/release/task/announce.md",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "parent must be supplied at most once",
        ));

    assert!(
        !env.cue_store()
            .join("acme/widgets/release/plan/rollout.md")
            .exists()
    );
}

#[test]
fn add_rejects_non_canonical_parent_references() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for invalid in [
        "/home/operator/cue/acme/widgets/release/task/publish.md",
        "~/cue/acme/widgets/release/task/publish.md",
        "task/publish.md",
    ] {
        env.command()
            .args([
                "add",
                "rollout",
                "Rollout plan",
                "--type",
                "plan",
                "--context",
                "release",
                "--frontmatter",
                &format!("parent={invalid}"),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("<org>/<repo>/<context>"));
    }

    assert!(
        !env.cue_store()
            .join("acme/widgets/release/plan/rollout.md")
            .exists()
    );
}

#[test]
fn add_rejects_non_canonical_ref_references() {
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
            "refs=acme/widgets/release/task/publish.md",
            "--frontmatter",
            "refs=note/rollout.md",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "too short to be a canonical address",
        ));

    assert!(
        !env.cue_store()
            .join("acme/widgets/release/spec/requirements.md")
            .exists()
    );
}

#[test]
fn add_accepts_nested_canonical_references() -> anyhow::Result<()> {
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
            "--frontmatter",
            "parent=acme/widgets/release",
            "--frontmatter",
            "refs=acme/widgets/release/note/ideas/canonical-addresses.md",
        ])
        .assert()
        .success();

    let metadata = read_frontmatter(
        &env.cue_store()
            .join("acme/widgets/release/note/ideas/rollout.md"),
    )?;
    assert_eq!(metadata["parent"], "acme/widgets/release");
    assert_eq!(
        metadata["refs"].as_sequence().map(Vec::as_slice),
        Some(
            &[Value::String(
                "acme/widgets/release/note/ideas/canonical-addresses.md".into()
            )][..]
        )
    );

    Ok(())
}
