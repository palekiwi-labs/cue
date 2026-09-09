mod helpers;

use std::process::Command;

#[test]
fn switch_sets_the_current_branch_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "switch", "release"])
        .assert()
        .success()
        .stdout("switched branch 'main' to context 'release'\n");

    let output = Command::new("git")
        .args(["config", "--local", "--get", "branch.main.cue-context"])
        .current_dir(env.root())
        .output()
        .expect("Failed to read branch context");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "release");
}
