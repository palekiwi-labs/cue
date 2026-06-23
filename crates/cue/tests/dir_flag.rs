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

/// A relative path (e.g. `../other`) is resolved correctly.
#[test]
fn test_dir_flag_relative_path() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo_empty(&cwd_env);

    let target_env = helpers::TestEnv::new();
    setup_repo_with_artifact(&target_env);

    // Build a relative path from cwd_env to target_env using ".."
    // Both are in temp dirs; construct a relative path via the common
    // tmpfs parent. We use "../<target_basename>" as a pragmatic
    // relative reference that resolves to the target root.
    let cwd_root = cwd_env.root().canonicalize()?;
    let target_root = target_env.root().canonicalize()?;
    let target_name = target_root.file_name().unwrap();
    let relative = std::path::PathBuf::from("..").join(target_name);

    let out = String::from_utf8(
        cwd_env
            .command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .current_dir(&cwd_root)
            .arg("--dir")
            .arg(&relative)
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        out.contains("index.md"),
        "relative --dir should resolve to target project"
    );

    Ok(())
}

/// A valid directory that is not a git repo produces a git error
/// downstream (not an unhandled panic). This documents the UX.
#[test]
fn test_dir_flag_non_git_directory_errors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    // env.root() is a plain temp dir — not a git repo.
    env.command()
        .arg("--dir")
        .arg(env.root())
        .arg("list")
        .assert()
        .failure();

    Ok(())
}

/// The flag is accepted *after* the subcommand (guards `global = true`).
#[test]
fn test_dir_flag_accepted_after_subcommand() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo_empty(&cwd_env);

    let target_env = helpers::TestEnv::new();
    setup_repo_with_artifact(&target_env);

    let out = String::from_utf8(
        cwd_env
            .command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--dir")
            .arg(target_env.root())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        out.contains("index.md"),
        "--dir after subcommand should still target the given project"
    );

    Ok(())
}

/// `cue --dir <path> add` writes the artifact into the target
/// project, not the CWD project.
#[test]
fn test_dir_flag_add_writes_to_target() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo_empty(&cwd_env);

    let target_env = helpers::TestEnv::new();
    setup_repo_empty(&target_env);

    // Add an artifact into target via --dir.
    cwd_env
        .command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("--dir")
        .arg(target_env.root())
        .arg("add")
        .arg("--root")
        .arg("remote.md")
        .arg("written via --dir")
        .assert()
        .success();

    // Listing from cwd should be empty.
    let cwd_out = cwd_env
        .command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(cwd_out.is_empty(), "CWD project should have no artifacts");

    // Listing from target should show the artifact.
    let target_out = String::from_utf8(
        target_env
            .command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        target_out.contains("remote.md"),
        "artifact should land in the target project"
    );

    Ok(())
}
