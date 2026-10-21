mod helpers;

use predicates::prelude::*;

#[test]
fn configured_default_scope_and_explicit_override() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for address in ["acme/widgets/alpha", "other/project/beta"] {
        env.command()
            .args(["context", "pin", address])
            .assert()
            .success();
    }
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{"default_scope":"store"}"#,
    )
    .unwrap();
    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nother/project/beta\n");
    env.command()
        .args(["context", "pins", "--scope", "repo"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\n");
}

#[test]
fn collections_reject_unknown_config_but_singular_commands_ignore_it() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    std::fs::write(env.config_dir.join("cue.json"), r#"{"branch_name":"old"}"#).unwrap();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["status", "--context", "alpha"])
        .assert()
        .success();
    for command in ["list", "pins"] {
        env.command()
            .args(["context", command])
            .assert()
            .failure()
            .stderr(predicate::str::contains("branch_name"));
    }
}
