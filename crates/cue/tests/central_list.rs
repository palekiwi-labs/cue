mod helpers;

use predicates::prelude::*;

#[test]
fn list_reads_artifacts_from_an_explicit_central_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "decisions",
            "Release decisions",
            "--type",
            "note",
            "--task",
            "release",
        ])
        .assert()
        .success();

    env.command()
        .args(["list", "--task", "release"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            env.cue_home()
                .join("acme/widgets/release/note/decisions.md")
                .to_string_lossy()
                .as_ref(),
        ));
}
