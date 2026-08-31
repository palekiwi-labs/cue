mod helpers;

use predicates::prelude::*;
use std::fs;

#[test]
fn test_log_add_links_to_trace_artifact() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("--root")
        .arg("error.log")
        .arg("stack trace content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Investigated failure")
        .arg("--trace")
        .arg(".test-mem/master/trace/error.log")
        .assert()
        .success();

    let content = fs::read_to_string(env.root().join(".test-mem/master/log.md"))?;
    assert!(content.contains("[error.log](trace/error.log)"));
    assert!(!content.contains("stack trace content"));

    Ok(())
}

#[test]
fn test_log_add_rejects_invalid_trace_references() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let assert_invalid = |reference: &str, message: &str| {
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("log")
            .arg("add")
            .arg("--title")
            .arg("Invalid trace")
            .arg("--trace")
            .arg(reference)
            .assert()
            .failure()
            .stderr(predicate::str::contains(message));
    };

    assert_invalid(
        ".test-mem/master/trace/missing.log",
        "Trace reference does not exist",
    );

    fs::create_dir_all(env.root().join(".test-mem/master/trace"))?;
    assert_invalid(
        ".test-mem/master/trace",
        "Trace reference must target a file",
    );

    let spec_path = env.root().join(".test-mem/master/spec/index.md");
    fs::create_dir_all(spec_path.parent().expect("spec path has parent"))?;
    fs::write(&spec_path, "spec")?;
    assert_invalid(
        ".test-mem/master/spec/index.md",
        "must target a trace artifact in scope 'master'",
    );

    let other_trace = env.root().join(".test-mem/other/trace/error.log");
    fs::create_dir_all(other_trace.parent().expect("trace path has parent"))?;
    fs::write(&other_trace, "other trace")?;
    assert_invalid(
        ".test-mem/other/trace/error.log",
        "must target a trace artifact in scope 'master'",
    );

    let outside = env.root().join("outside.log");
    fs::write(&outside, "outside")?;
    assert_invalid(
        ".test-mem/master/trace/../../../outside.log",
        "resolves outside the cue store",
    );
    assert_invalid(
        outside.to_str().expect("temporary path is UTF-8"),
        "resolves outside the cue store",
    );

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            &outside,
            env.root().join(".test-mem/master/trace/escaped.log"),
        )?;
        assert_invalid(
            ".test-mem/master/trace/escaped.log",
            "resolves outside the cue store",
        );
    }

    Ok(())
}

#[test]
fn test_log_add_basic() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a log entry
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Test Title")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/master/log.md\n"));

    let log_path = env.root().join(".test-mem/master/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("# Project Log"));
    assert!(content.contains("Test Title"));

    // Add another log entry with dirty tree
    fs::write(env.root().join("dirty.txt"), "dirty")?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Dirty Entry")
        .arg("--found")
        .arg("Found something")
        .arg("--decided")
        .arg("Decided something")
        .assert()
        .success();

    let content = fs::read_to_string(&log_path)?;
    assert!(content.contains("-dirty] Dirty Entry"));
    assert!(content.contains("- **Found:** Found something"));
    assert!(content.contains("- **Decided:** Decided something"));

    Ok(())
}

#[test]
fn test_log_add_from_file() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("--root")
        .arg("json.log")
        .arg("JSON trace contents")
        .assert()
        .success();

    let json_content = r#"{
        "title": "JSON Title",
        "trace": ".test-mem/master/trace/json.log",
        "open": ["Question 1", "Question 2"]
    }"#;
    let json_path = env.root().join("log.json");
    fs::write(&json_path, json_content)?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--file")
        .arg(&json_path)
        .assert()
        .success();

    let log_path = env.root().join(".test-mem/master/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("JSON Title"));
    assert!(content.contains("[json.log](trace/json.log)"));
    assert!(!content.contains("JSON trace contents"));
    assert!(content.contains("- **Open:** Question 1"));
    assert!(content.contains("- **Open:** Question 2"));

    Ok(())
}

#[test]
fn test_log_add_validation() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Empty title
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("   ")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Title cannot be empty"));

    // Missing title
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .assert()
        .failure()
        .stderr(predicate::str::contains("The --title argument is required"));

    // Removed body argument
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Some title")
        .arg("--body")
        .arg("Some body")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--body'"));

    // Empty task override
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Some Title")
        .arg("--task")
        .arg("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid task slug"));

    Ok(())
}

#[test]
fn test_log_add_branch_and_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Write entry to task scope "feature-other"
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Round Trip")
        .arg("--task")
        .arg("feature-other")
        .assert()
        .success();

    // Read it back with log list --task
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .arg("--task")
        .arg("feature-other")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Project Log"))
        .stdout(predicate::str::contains("Round Trip"));

    Ok(())
}

#[test]
fn test_log_add_file_with_branch() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let json_content = r#"{"title": "File Branch Title"}"#;
    let json_path = env.root().join("entry.json");
    fs::write(&json_path, json_content)?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--file")
        .arg(&json_path)
        .arg("--task")
        .arg("feature-other")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/feature-other/log.md\n"));

    let log_path = env.root().join(".test-mem/feature-other/log.md");
    let content = fs::read_to_string(&log_path)?;
    assert!(content.contains("File Branch Title"));

    Ok(())
}

#[test]
fn test_log_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Uninitialized
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cue init"));

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Initialized but no log
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Add entry
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("My Title")
        .assert()
        .success();

    // 3. Has log
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Project Log"))
        .stdout(predicate::str::contains("My Title"));

    Ok(())
}

#[test]
fn test_log_add_with_explicit_branch() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a log entry to a DIFFERENT scope than current (master)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("log")
        .arg("add")
        .arg("--title")
        .arg("Branch Entry")
        .arg("--task")
        .arg("feature-other")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/feature-other/log.md\n"));

    let log_path = env.root().join(".test-mem/feature-other/log.md");
    let content = fs::read_to_string(&log_path)?;

    assert!(content.contains("# Project Log"));
    assert!(content.contains("Branch Entry"));

    // Verify main branch log does not have this entry
    let main_log = env.root().join(".test-mem/master/log.md");
    assert!(!main_log.exists());

    Ok(())
}
