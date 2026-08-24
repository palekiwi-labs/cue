mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;
use std::process::Command;

fn git(env: &TestEnv, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(env.root())
        .output()
        .expect("failed to spawn git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn branch_task(env: &TestEnv, branch: &str) -> Option<String> {
    let out = Command::new("git")
        .args([
            "config",
            "--local",
            "--get",
            &format!("branch.{}.cue-task", branch),
        ])
        .current_dir(env.root())
        .output()
        .expect("failed to spawn git");
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn setup_on_branch(branch: &str) -> TestEnv {
    let env = TestEnv::new();
    helpers::setup_git_repo(env.root());
    cue(&env).arg("init").assert().success();
    if branch != "main" {
        git(&env, &["checkout", "-b", branch]);
    }
    env
}

fn cue(env: &TestEnv) -> assert_cmd::Command {
    let mut cmd = env.command();
    cmd.env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem");
    cmd
}

#[test]
fn switch_on_branch_writes_association() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");

    cue(&env)
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"));

    assert_eq!(
        branch_task(&env, "feat/auth").as_deref(),
        Some("auth-login")
    );

    Ok(())
}

#[test]
fn switch_to_master_clears_association() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");

    cue(&env).arg("switch").arg("auth-login").assert().success();
    assert!(branch_task(&env, "feat/auth").is_some());

    cue(&env)
        .arg("switch")
        .arg("master")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to global context\n"));

    assert!(
        branch_task(&env, "feat/auth").is_none(),
        "switching to master must clear the association"
    );

    Ok(())
}

#[test]
fn switch_no_args_restores_associated_task() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");

    cue(&env).arg("switch").arg("auth-login").assert().success();
    // The association lives in git config, surviving checkouts away and back.
    git(&env, &["checkout", "main"]);
    git(&env, &["checkout", "feat/auth"]);

    cue(&env)
        .arg("switch")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"));

    let head = std::fs::read_to_string(env.root().join(".test-mem/HEAD"))?;
    assert_eq!(head.trim(), "auth-login");

    Ok(())
}

#[test]
fn switch_no_args_without_association_fails() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");

    cue(&env)
        .arg("switch")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no task associated with branch 'feat/auth'",
        ));

    Ok(())
}

#[test]
fn switch_no_args_on_detached_head_fails() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");
    git(&env, &["checkout", "--detach"]);

    cue(&env)
        .arg("switch")
        .assert()
        .failure()
        .stderr(predicate::str::contains("detached"));

    Ok(())
}

#[test]
fn switch_on_detached_head_skips_association() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");
    git(&env, &["checkout", "--detach"]);

    cue(&env)
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"));

    assert!(
        branch_task(&env, "feat/auth").is_none(),
        "no association may be written from a detached HEAD"
    );

    Ok(())
}

#[test]
fn dotted_branch_name_round_trips() -> anyhow::Result<()> {
    let env = setup_on_branch("release/v1.0.0");

    cue(&env)
        .arg("switch")
        .arg("release-polish")
        .assert()
        .success();
    assert_eq!(
        branch_task(&env, "release/v1.0.0").as_deref(),
        Some("release-polish")
    );

    cue(&env).arg("switch").arg("master").assert().success();
    assert!(
        branch_task(&env, "release/v1.0.0").is_none(),
        "switching to master must clear the dotted-branch association"
    );

    cue(&env)
        .arg("switch")
        .arg("release-polish")
        .assert()
        .success();
    cue(&env)
        .arg("switch")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: release-polish\n"));

    Ok(())
}

#[test]
fn association_write_failure_warns_but_switch_succeeds() -> anyhow::Result<()> {
    let env = setup_on_branch("feat/auth");

    // A stale config lock makes every `git config` write fail.
    std::fs::write(env.root().join(".git/config.lock"), b"stale")?;

    cue(&env)
        .arg("switch")
        .arg("auth-login")
        .assert()
        .success()
        .stdout(predicate::str::diff("switched to task: auth-login\n"))
        .stderr(predicate::str::contains("warning:"));

    let head = std::fs::read_to_string(env.root().join(".test-mem/HEAD"))?;
    assert_eq!(head.trim(), "auth-login");

    Ok(())
}
