mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::fs;

// -- existing tests (updated: scope resolves from HEAD, not git branch) -------

#[test]
fn test_context_render_with_globs() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // With no HEAD set, scope falls back to "master".
    let scope_dir = env.root().join(".cue").join("master");
    let spec_dir = scope_dir.join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("1.md"), "content 1")?;
    fs::write(spec_dir.join("2.md"), "content 2")?;

    fs::write(
        scope_dir.join("context.json"),
        r#"{
        "default": {
            "artifacts": ["./spec/*.md"]
        }
    }"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("content 1"))
        .stdout(predicate::str::contains("content 2"))
        .stdout(predicate::str::contains("path=\"master/spec/1.md\""))
        .stdout(predicate::str::contains("path=\"master/spec/2.md\""));

    Ok(())
}

#[test]
fn test_context_render_instructions() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // With no HEAD set, scope falls back to "master".
    let scope_dir = env.root().join(".cue").join("master");
    fs::create_dir_all(&scope_dir)?;
    fs::write(
        scope_dir.join("context.json"),
        r#"{
        "default": {
            "artifacts": [],
            "instructions": "Please implement the feature"
        }
    }"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "<instructions>\nPlease implement the feature\n</instructions>",
        ));

    Ok(())
}

// -- new tests: HEAD-derived scope --------------------------------------------

/// When .cue/HEAD contains a task slug, render uses that task's context.json.
#[test]
fn test_context_render_uses_head_scope() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    let cue_dir = env.root().join(".cue");
    let task_dir = cue_dir.join("my-task");
    let spec_dir = task_dir.join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("task-file.md"), "task scoped content")?;
    fs::write(
        task_dir.join("context.json"),
        r#"{
        "default": {
            "artifacts": ["./spec/task-file.md"]
        }
    }"#,
    )?;

    // Point HEAD at the task slug.
    fs::write(cue_dir.join("HEAD"), "my-task")?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("task scoped content"))
        .stdout(predicate::str::contains(
            "path=\"my-task/spec/task-file.md\"",
        ));

    Ok(())
}

// -- config-default fallback tests -------------------------------------------

/// render produces output when context.json is absent but config.context
/// supplies the profile (review #11).
#[test]
fn test_context_render_uses_config_default_when_no_context_json() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Write a real artifact under the master scope directory.
    let master_dir = env.root().join(".cue").join("master");
    let spec_dir = master_dir.join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("index.md"), "hello from config default")?;

    // No context.json — the profile lives in cue.json instead.
    fs::write(
        env.root().join("cue.json"),
        r#"{"context": {"default": {"artifacts": ["./spec/index.md"]}}}"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stderr(predicate::str::contains("no context.json"))
        .stdout(predicate::str::contains("hello from config default"));

    Ok(())
}

/// A config-default profile that includes an on-disk scope resolves that
/// scope's artifacts correctly (review #12).
#[test]
fn test_context_render_config_default_with_include() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // "master" scope has no context.json — profile comes from cue.json.
    // The profile includes "@other-task".
    let other_dir = env.root().join(".cue").join("other-task");
    let other_spec = other_dir.join("spec");
    fs::create_dir_all(&other_spec)?;
    fs::write(other_spec.join("other.md"), "from included scope")?;
    fs::write(
        other_dir.join("context.json"),
        r#"{"default": {"artifacts": ["./spec/other.md"]}}"#,
    )?;

    fs::write(
        env.root().join("cue.json"),
        r#"{"context": {"default": {"include": ["@other-task"], "artifacts": []}}}"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("from included scope"));

    Ok(())
}

/// A config-default profile with a glob pattern expands correctly
/// (review #13).
#[test]
fn test_context_render_config_default_with_glob() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    let master_dir = env.root().join(".cue").join("master");
    let spec_dir = master_dir.join("spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("a.md"), "file a")?;
    fs::write(spec_dir.join("b.md"), "file b")?;

    // No context.json; glob pattern comes from config default.
    fs::write(
        env.root().join("cue.json"),
        r#"{"context": {"default": {"artifacts": ["./spec/*.md"]}}}"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("file a"))
        .stdout(predicate::str::contains("file b"));

    Ok(())
}

/// When .cue/HEAD is absent, render falls back to the "master" scope,
/// not the git branch name.
#[test]
fn test_context_render_no_head_falls_back_to_master() -> anyhow::Result<()> {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());

    // The git branch is "main" (set by setup_git_repo). We deliberately place
    // content under "master/" only. After the fix, render must use "master",
    // not "main". No .cue/HEAD is written.
    let master_dir = env.root().join(".cue").join("master");
    fs::create_dir_all(&master_dir)?;
    fs::write(
        master_dir.join("context.json"),
        r#"{
        "default": {
            "artifacts": [],
            "instructions": "master fallback"
        }
    }"#,
    )?;

    env.command()
        .arg("context")
        .arg("render")
        .assert()
        .success()
        .stdout(predicate::str::contains("master fallback"));

    Ok(())
}
