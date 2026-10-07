mod helpers;

use std::process::Command;

fn git(env: &helpers::TestEnv, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(env.root())
        .output()
        .expect("Failed to run git")
}

fn branch_context(env: &helpers::TestEnv, branch: &str) -> Option<String> {
    let key = format!("branch.{branch}.cue-context");
    let output = git(env, &["config", "--local", "--get", &key]);
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[test]
fn switch_sets_the_current_branch_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "switch", "../release"])
        .assert()
        .success()
        .stdout("switched branch 'main' to context '../release'\n");

    assert_eq!(branch_context(&env, "main").as_deref(), Some("../release"));
    assert!(!env.root().join(".cue").exists());
    assert_eq!(
        std::fs::read_dir(env.cue_store())
            .expect("Failed to read CUE_STORE")
            .count(),
        0
    );
}

#[test]
fn switch_configures_an_explicit_branch_from_detached_head() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(git(&env, &["checkout", "--detach"]).status.success());

    env.command()
        .args(["context", "switch", "release", "--branch", "future"])
        .assert()
        .success()
        .stdout("switched branch 'future' to context 'release'\n");

    assert_eq!(branch_context(&env, "future").as_deref(), Some("release"));
}

#[test]
fn switch_requires_an_explicit_branch_in_detached_head() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(git(&env, &["checkout", "--detach"]).status.success());

    env.command()
        .args(["context", "switch", "release"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "cannot switch context in detached HEAD; specify target branch with --branch <name>",
        ));
}

#[test]
fn unset_clears_the_current_branch_context_idempotently() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(
        git(
            &env,
            ["config", "--local", "branch.main.cue-context", "release",].as_slice(),
        )
        .status
        .success()
    );

    for _ in 0..2 {
        env.command()
            .args(["context", "unset"])
            .assert()
            .success()
            .stdout("unset context for branch 'main'\n");
        assert_eq!(branch_context(&env, "main"), None);
    }
}

#[test]
fn unset_clears_an_explicit_branch_from_detached_head() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(
        git(
            &env,
            ["config", "--local", "branch.future.cue-context", "release",].as_slice(),
        )
        .status
        .success()
    );
    assert!(git(&env, &["checkout", "--detach"]).status.success());

    env.command()
        .args(["context", "unset", "--branch", "future"])
        .assert()
        .success()
        .stdout("unset context for branch 'future'\n");

    assert_eq!(branch_context(&env, "future"), None);
}

#[test]
fn unset_requires_an_explicit_branch_in_detached_head() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(git(&env, &["checkout", "--detach"]).status.success());

    env.command()
        .args(["context", "unset"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "cannot unset context in detached HEAD; specify target branch with --branch <name>",
        ));
}

#[test]
fn switched_context_is_observed_by_status() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args(["context", "switch", "release"])
        .assert()
        .success();

    let output = env
        .command()
        .args(["status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(status["context"], "release");

    env.command().args(["context", "unset"]).assert().success();
    let output = env
        .command()
        .args(["status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: serde_json::Value = serde_json::from_slice(&output)?;
    assert!(status["context"].is_null());

    Ok(())
}

#[test]
fn switch_requires_a_slug_and_unset_rejects_one() {
    let env = helpers::TestEnv::new();

    env.command()
        .args(["context", "switch"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("<SLUG>"));
    env.command()
        .args(["context", "unset", "release"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unexpected argument 'release'"));
}

#[test]
fn unset_with_an_explicit_branch_still_requires_a_git_repository() {
    let env = helpers::TestEnv::new();

    env.command()
        .args(["context", "unset", "--branch", "future"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a git repository"));
}
