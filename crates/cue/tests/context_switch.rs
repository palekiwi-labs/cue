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
        .args(["context", "switch", "release"])
        .assert()
        .success()
        .stdout("switched branch 'main' to context 'release'\n");

    assert_eq!(branch_context(&env, "main").as_deref(), Some("release"));
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
