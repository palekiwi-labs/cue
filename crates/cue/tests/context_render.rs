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
        .stdout(predicate::str::contains("path=\".cue/master/spec/1.md\""))
        .stdout(predicate::str::contains("path=\".cue/master/spec/2.md\""));

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
            "path=\".cue/my-task/spec/task-file.md\"",
        ));

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
