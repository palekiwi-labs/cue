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

    assert_eq!(metadata["status"], "open");
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

/// A bin artifact is a script, so the fixtures are scripts. The shebang is
/// the portable `/usr/bin/env` form because cue stores what it is given.
const SCRIPT: &str = "#!/usr/bin/env bash\nset -euo pipefail\necho ready\n";

/// The permissions a plain `std::fs::write` produces in `dir`, which is the
/// umask-derived baseline every artifact type shares. Deriving the expectation
/// from the running environment keeps the assertion true under any umask
/// instead of hardcoding one developer's 0o644.
#[cfg(unix)]
fn baseline_mode(dir: &Path) -> anyhow::Result<u32> {
    use std::os::unix::fs::PermissionsExt;

    let probe = dir.join("umask-probe");
    std::fs::write(&probe, b"probe")?;
    let mode = std::fs::metadata(&probe)?.permissions().mode() & 0o777;
    std::fs::remove_file(&probe)?;
    Ok(mode)
}

#[cfg(unix)]
fn mode_of(path: &Path) -> anyhow::Result<u32> {
    use std::os::unix::fs::PermissionsExt;

    Ok(std::fs::metadata(path)?.permissions().mode() & 0o777)
}

#[test]
fn add_stores_bin_scripts_verbatim_under_the_given_name() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
    assert_eq!(std::fs::read(&path)?, SCRIPT.as_bytes());
    assert!(
        !env.cue_store()
            .join("acme/widgets/release/bin/smoke.sh.json")
            .exists(),
        "a bin artifact must not be renamed into a JSON document"
    );

    Ok(())
}

#[test]
fn add_keeps_extensionless_bin_names_extensionless() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let bin_dir = env.cue_store().join("acme/widgets/release/bin");
    assert_eq!(std::fs::read(bin_dir.join("smoke"))?, SCRIPT.as_bytes());
    assert!(
        !bin_dir.join("smoke.json").exists(),
        "an extensionless bin name must not gain a default extension"
    );

    Ok(())
}

#[test]
fn add_preserves_non_utf8_bin_content_from_stdin() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    let bytes: Vec<u8> = vec![0x7f, b'E', b'L', b'F', 0x02, 0x00, 0xff, 0xfe, 0x00, 0x80];
    env.command()
        .args(["add", "probe", "-", "--type", "bin", "--context", "release"])
        .write_stdin(bytes.clone())
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/bin/probe");
    assert_eq!(std::fs::read(&path)?, bytes);

    Ok(())
}

#[cfg(unix)]
#[test]
fn add_marks_bin_artifacts_executable_for_readable_classes() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let baseline = baseline_mode(env.root())?;
    let expected = baseline | ((baseline & 0o444) >> 2);
    let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
    assert_eq!(
        mode_of(&path)?,
        expected,
        "a bin artifact gains execute for exactly the classes that may read it"
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn add_leaves_other_artifact_types_non_executable() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for (filename, content, cue_type, relative) in [
        ("publish", "Publish the release", "task", "task/publish.md"),
        (
            "readiness",
            r#"{"verdict":"approve"}"#,
            "review",
            "review/readiness.json",
        ),
    ] {
        env.command()
            .args([
                "add",
                filename,
                content,
                "--type",
                cue_type,
                "--context",
                "release",
            ])
            .assert()
            .success();

        let path = env.cue_store().join("acme/widgets/release").join(relative);
        assert_eq!(
            mode_of(&path)? & 0o111,
            0,
            "{cue_type} artifacts are documents, not executables"
        );
    }

    Ok(())
}

#[test]
fn add_creates_bin_artifacts_in_nested_directories() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "checks/readiness.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/bin/checks/readiness.sh");
    assert_eq!(std::fs::read(&path)?, SCRIPT.as_bytes());
    #[cfg(unix)]
    {
        let baseline = baseline_mode(env.root())?;
        assert_eq!(mode_of(&path)?, baseline | ((baseline & 0o444) >> 2));
    }

    Ok(())
}

#[test]
fn add_rejects_unsafe_bin_paths() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for (filename, message) in [
        ("../escape.sh", "'..' is not allowed"),
        ("/etc/cron.sh", "absolute paths are not allowed"),
        ("checks/", "trailing path separators are not allowed"),
    ] {
        env.command()
            .args([
                "add",
                filename,
                SCRIPT,
                "--type",
                "bin",
                "--context",
                "release",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains(message));
    }
}

#[test]
fn add_rejects_bin_metadata() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
            "--frontmatter",
            "analyzer=smoke-test",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "bin artifacts do not support metadata",
        ));

    assert!(
        !env.cue_store()
            .join("acme/widgets/release/bin/smoke.sh")
            .exists(),
        "a rejected bin write must leave nothing behind"
    );
}

#[test]
fn add_rejects_bin_metadata_before_overwriting_with_force() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            "#!/usr/bin/env bash\necho replaced\n",
            "--type",
            "bin",
            "--context",
            "release",
            "--force",
            "--frontmatter",
            "analyzer=smoke-test",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "bin artifacts do not support metadata",
        ));

    let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
    assert_eq!(
        std::fs::read(&path)?,
        SCRIPT.as_bytes(),
        "rejecting metadata must not disturb the existing artifact"
    );

    Ok(())
}

#[test]
fn add_refuses_to_replace_an_existing_bin_without_force() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            "#!/usr/bin/env bash\necho replaced\n",
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("File exists"));

    let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
    assert_eq!(std::fs::read(&path)?, SCRIPT.as_bytes());

    Ok(())
}

#[test]
fn add_replaces_an_existing_bin_with_force() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let replacement = "#!/usr/bin/env bash\necho replaced\n";
    env.command()
        .args([
            "add",
            "smoke.sh",
            replacement,
            "--type",
            "bin",
            "--context",
            "release",
            "--force",
        ])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
    assert_eq!(std::fs::read(&path)?, replacement.as_bytes());
    #[cfg(unix)]
    {
        let baseline = baseline_mode(env.root())?;
        assert_eq!(
            mode_of(&path)?,
            baseline | ((baseline & 0o444) >> 2),
            "a replaced bin artifact stays executable"
        );
    }

    Ok(())
}

#[cfg(unix)]
#[test]
fn force_keeps_the_permissions_the_replaced_bin_carried() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    // A narrowed artifact stays narrowed across a replacement: the operator's
    // `chmod` is the base the execute bits are derived from, so a private
    // script is not quietly reopened to the umask default.
    for (existing, expected) in [(0o600, 0o700), (0o700, 0o700)] {
        env.command()
            .args([
                "add",
                "smoke.sh",
                SCRIPT,
                "--type",
                "bin",
                "--context",
                "release",
                "--force",
            ])
            .assert()
            .success();

        let path = env.cue_store().join("acme/widgets/release/bin/smoke.sh");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(existing))?;

        let replacement = "#!/usr/bin/env bash\necho replaced\n";
        env.command()
            .args([
                "add",
                "smoke.sh",
                replacement,
                "--type",
                "bin",
                "--context",
                "release",
                "--force",
            ])
            .assert()
            .success();

        assert_eq!(std::fs::read(&path)?, replacement.as_bytes());
        assert_eq!(
            mode_of(&path)?,
            expected,
            "replacing a {existing:o} artifact must publish {expected:o}"
        );
    }

    Ok(())
}

#[cfg(unix)]
#[test]
fn add_refuses_a_bin_name_that_is_already_taken_by_a_dangling_link() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    // `Path::exists` follows the link and reports absence, so this name slips
    // past the pre-check: only the no-clobber publication stops cue writing
    // through a name that is already taken.
    let bin_dir = env.cue_store().join("acme/widgets/release/bin");
    std::fs::create_dir_all(&bin_dir)?;
    let target = bin_dir.join("missing");
    let link = bin_dir.join("smoke.sh");
    std::os::unix::fs::symlink(&target, &link)?;

    env.command()
        .args([
            "add",
            "smoke.sh",
            SCRIPT,
            "--type",
            "bin",
            "--context",
            "release",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("File exists"));

    assert!(
        std::fs::symlink_metadata(&link)?.file_type().is_symlink(),
        "the existing name must be left exactly as it was"
    );
    assert!(!target.exists(), "nothing may be written through the link");

    Ok(())
}

#[test]
fn add_creates_json_review_artifacts() -> anyhow::Result<()> {
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
            r#"{"findings":[{"severity":"blocker"}]}"#,
            "--type",
            "review",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/review/readiness.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;

    assert_eq!(content["findings"][0]["severity"], "blocker");

    Ok(())
}

#[test]
fn add_rejects_review_content_that_is_not_a_json_object() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    for content in ["not json at all", r#"["findings"]"#, r#""blocker""#] {
        env.command()
            .args([
                "add",
                "readiness",
                content,
                "--type",
                "review",
                "--context",
                "release",
            ])
            .assert()
            .failure()
            // Asserting the writer's own complaint distinguishes a rejected
            // payload from a `review` type clap never accepted in the first
            // place, which would otherwise satisfy this test vacuously.
            .stderr(predicate::str::contains("review content must be"));
    }

    assert!(
        !env.cue_store()
            .join("acme/widgets/release/review/readiness.json")
            .exists()
    );
}

#[test]
fn add_accepts_nested_review_paths_and_any_extension() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    // An extensionless name defaults to `.json`, while a caller-supplied
    // extension is taken as given: cue stores the object, it does not police
    // what the file is called.
    for (filename, relative) in [
        ("rounds/first", "rounds/first.json"),
        ("rounds/second.review", "rounds/second.review"),
    ] {
        env.command()
            .args([
                "add",
                filename,
                r#"{"verdict":"approve"}"#,
                "--type",
                "review",
                "--context",
                "release",
            ])
            .assert()
            .success();

        let path = env
            .cue_store()
            .join("acme/widgets/release/review")
            .join(relative);
        let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        assert_eq!(content["verdict"], "approve");
    }

    Ok(())
}

#[test]
fn add_does_not_stamp_review_artifacts() -> anyhow::Result<()> {
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
            r#"{"verdict":"approve"}"#,
            "--type",
            "review",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/review/readiness.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;

    // A review carries only what its caller supplied. cue adds no lifecycle
    // or revision correlation of its own.
    assert!(content.get("created_at").is_none());
    assert!(content.get("commit_hash").is_none());
    assert!(content.get("repo_id").is_none());
    assert_eq!(content.as_object().map(serde_json::Map::len), Some(1));

    Ok(())
}

#[test]
fn add_merges_review_metadata_into_existing_json_fields() -> anyhow::Result<()> {
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
            r#"{"reviewer":"ada","verdict":"approve"}"#,
            "--type",
            "review",
            "--context",
            "release",
            "--frontmatter",
            "reviewer=grace",
            "--frontmatter",
            "round=2",
        ])
        .assert()
        .success();

    let path = env
        .cue_store()
        .join("acme/widgets/release/review/readiness.json");
    let content: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;

    // Merging is inherited wholesale from the JSON writer: a key already in
    // the payload is promoted to a list rather than overwritten, and a scalar
    // is coerced rather than stored as a string.
    assert_eq!(content["reviewer"], serde_json::json!(["ada", "grace"]));
    assert_eq!(content["round"], serde_json::json!(2));
    assert_eq!(content["verdict"], "approve");

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

    // A nested artifact tail is the same shape as a context address, so only
    // the store can tell them apart.
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
            "refs=note/ideas/rollout.md",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no context 'note/ideas/rollout.md' in the store",
        ));

    // An address naming a context that does not exist is equally unusable.
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
            "refs=acme/widgets/unknown/task/publish.md",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no context 'acme/widgets/unknown' in the store",
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
