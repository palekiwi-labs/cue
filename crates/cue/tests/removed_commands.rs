mod helpers;

use predicates::prelude::*;

#[test]
fn switch_command_is_removed() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("switch")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand 'switch'"));
}

#[test]
fn config_command_is_removed() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("config")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand 'config'"));
}

#[test]
fn project_command_is_removed() {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("project")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unrecognized subcommand 'project'",
        ));
}

#[test]
fn legacy_context_commands_are_removed() {
    for command in ["init", "show", "profiles", "render", "path"] {
        let env = helpers::TestEnv::new();

        env.command()
            .args(["context", command])
            .assert()
            .failure()
            .stderr(predicate::str::contains(format!(
                "unrecognized subcommand '{command}'"
            )));
    }
}
