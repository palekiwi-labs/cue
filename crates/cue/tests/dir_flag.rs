mod helpers;

use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const CWD_ORIGIN: &str = "https://github.com/acme/cwd.git";
const TARGET_ORIGIN: &str = "https://github.com/acme/target.git";

fn setup_repo(env: &helpers::TestEnv, origin: &str) {
    env.setup_repo_with_origin();
    let output = Command::new("git")
        .args(["remote", "set-url", "origin", origin])
        .current_dir(env.root())
        .output()
        .expect("failed to set fixture origin");
    assert!(
        output.status.success(),
        "failed to set fixture origin: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn status_scope(
    cwd_env: &helpers::TestEnv,
    store: &Path,
    dir_flag: &str,
    target: &Path,
) -> anyhow::Result<String> {
    let output = cwd_env
        .command()
        .env("CUE_STORE", store)
        .arg(dir_flag)
        .arg(target)
        .args(["status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: Value = serde_json::from_slice(&output)?;
    Ok(status["scope"]
        .as_str()
        .expect("status should contain a repository scope")
        .to_owned())
}

#[test]
fn dir_flag_targets_given_repository() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo(&cwd_env, CWD_ORIGIN);
    let target_env = helpers::TestEnv::new();
    setup_repo(&target_env, TARGET_ORIGIN);

    assert_eq!(
        status_scope(&cwd_env, cwd_env.cue_store(), "--dir", target_env.root())?,
        "acme/target"
    );

    Ok(())
}

#[test]
fn short_alias_targets_given_repository() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo(&cwd_env, CWD_ORIGIN);
    let target_env = helpers::TestEnv::new();
    setup_repo(&target_env, TARGET_ORIGIN);

    assert_eq!(
        status_scope(&cwd_env, cwd_env.cue_store(), "-C", target_env.root())?,
        "acme/target"
    );

    Ok(())
}

#[test]
fn dir_flag_accepts_a_relative_path() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo(&cwd_env, CWD_ORIGIN);
    let target_env = helpers::TestEnv::new();
    setup_repo(&target_env, TARGET_ORIGIN);

    let target_root = target_env.root().canonicalize()?;
    let relative = Path::new("..").join(
        target_root
            .file_name()
            .expect("temporary directory should have a name"),
    );

    assert_eq!(
        status_scope(&cwd_env, cwd_env.cue_store(), "--dir", &relative)?,
        "acme/target"
    );

    Ok(())
}

#[test]
fn dir_flag_is_accepted_after_the_subcommand() -> anyhow::Result<()> {
    let cwd_env = helpers::TestEnv::new();
    setup_repo(&cwd_env, CWD_ORIGIN);
    let target_env = helpers::TestEnv::new();
    setup_repo(&target_env, TARGET_ORIGIN);

    let output = cwd_env
        .command()
        .env("CUE_STORE", cwd_env.cue_store())
        .args(["status", "--json", "--dir"])
        .arg(target_env.root())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: Value = serde_json::from_slice(&output)?;

    assert_eq!(status["scope"], "acme/target");

    Ok(())
}

#[test]
fn dir_flag_add_writes_to_target_repository_scope() {
    let cwd_env = helpers::TestEnv::new();
    setup_repo(&cwd_env, CWD_ORIGIN);
    let target_env = helpers::TestEnv::new();
    setup_repo(&target_env, TARGET_ORIGIN);
    let store = cwd_env.cue_store();

    cwd_env
        .command()
        .env("CUE_STORE", store)
        .arg("--dir")
        .arg(target_env.root())
        .arg("init")
        .assert()
        .success();
    cwd_env
        .command()
        .env("CUE_STORE", store)
        .arg("--dir")
        .arg(target_env.root())
        .args(["context", "create", "release"])
        .assert()
        .success();
    cwd_env
        .command()
        .env("CUE_STORE", store)
        .arg("--dir")
        .arg(target_env.root())
        .args([
            "add",
            "decisions",
            "Target repository decisions",
            "--type",
            "note",
            "--context",
            "release",
        ])
        .assert()
        .success();

    assert!(
        store
            .join("acme/target/release/note/decisions.md")
            .is_file()
    );
    assert!(!store.join("acme/cwd/release").exists());
}

#[test]
fn dir_flag_rejects_a_nonexistent_path() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("--dir")
        .arg("/this/path/does/not/exist/abc123")
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn dir_flag_rejects_a_file_path() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let file_path = env.root().join("not_a_dir.txt");
    std::fs::write(&file_path, "I am a file")?;

    env.command()
        .arg("--dir")
        .arg(&file_path)
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));

    Ok(())
}

#[test]
fn dir_flag_reports_a_non_git_directory_error() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("--dir")
        .arg(env.root())
        .arg("status")
        .assert()
        .failure();
}
