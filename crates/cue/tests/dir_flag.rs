mod helpers;

/// Integration tests for the global --dir / -C flag.
///
/// The flag overrides the process CWD for all subcommands so they
/// operate on the project at <PATH> rather than the directory from
/// which `cue` was invoked.

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Set up a git repo with a `.test-mem/` cue directory and one artifact.
fn setup_repo_with_artifact(env: &helpers::TestEnv) {
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
        .arg("--root")
        .arg("index.md")
        .arg("artifact in target project")
        .assert()
        .success();
}

/// Set up a git repo with a `.test-mem/` cue directory but no artifacts.
fn setup_repo_empty(env: &helpers::TestEnv) {
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// `cue --dir <path> list` lists artifacts in the project at <path>,
/// not in the CWD project.
#[test]
fn test_dir_flag_targets_given_path() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo_empty(&cwd_env);

    let target_env = helpers::TestEnv::new();
    setup_repo_with_artifact(&target_env);

    // Without --dir: lists from CWD project — should be empty.
    let out_without = cwd_env
        .command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(out_without.is_empty(), "CWD project has no artifacts");

    // With --dir: lists from target project — should contain the artifact.
    let out_with = String::from_utf8(
        cwd_env
            .command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("--dir")
            .arg(target_env.root())
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        out_with.contains("index.md"),
        "should list artifact from target project"
    );

    Ok(())
}

/// `-C` short alias behaves identically to `--dir`.
#[test]
fn test_short_alias_c() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo_empty(&cwd_env);

    let target_env = helpers::TestEnv::new();
    setup_repo_with_artifact(&target_env);

    let out = String::from_utf8(
        cwd_env
            .command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("-C")
            .arg(target_env.root())
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        out.contains("index.md"),
        "-C should list artifact from target project"
    );

    Ok(())
}

/// A path that does not exist produces a clear error, not a panic.
#[test]
fn test_dir_flag_nonexistent_path_errors() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("--dir")
        .arg("/this/path/does/not/exist/abc123")
        .arg("list")
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not exist"));
}

/// A path that points to a file (not a directory) produces a clear
/// error, not a panic.
#[test]
fn test_dir_flag_file_path_errors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let file_path = env.root().join("not_a_dir.txt");
    std::fs::write(&file_path, "I am a file")?;

    env.command()
        .arg("--dir")
        .arg(&file_path)
        .arg("list")
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a directory"));

    Ok(())
}
