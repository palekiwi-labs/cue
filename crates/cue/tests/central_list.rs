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

#[test]
fn list_without_an_active_context_reads_the_repository_scope() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();

    for (context, artifact) in [("release", "decisions"), ("hotfix", "incident")] {
        env.command()
            .args(["context", "create", context])
            .assert()
            .success();
        env.command()
            .args([
                "add", artifact, "Notes", "--type", "note", "--task", context,
            ])
            .assert()
            .success();
    }

    env.command()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("release/note/decisions.md"))
        .stdout(predicate::str::contains("hotfix/note/incident.md"));
}
