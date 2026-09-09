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

#[test]
fn legacy_artifact_options_are_removed() {
    for args in [
        vec!["add", "artifact", "body", "--root"],
        vec!["list", "--all"],
        vec!["list", "--include-gitignored"],
    ] {
        let env = helpers::TestEnv::new();

        env.command()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

#[test]
fn legacy_task_context_selector_is_removed() {
    for args in [
        vec!["status", "--task", "release"],
        vec!["add", "artifact", "body", "--task", "release"],
        vec!["list", "--task", "release"],
        vec!["log", "add", "--task", "release", "--title", "Entry"],
        vec!["log", "list", "--task", "release"],
    ] {
        let env = helpers::TestEnv::new();

        env.command()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unexpected argument '--task'"));
    }
}

#[test]
fn removed_artifact_types_are_rejected_at_the_cli_boundary() {
    let env = helpers::TestEnv::new();

    env.command()
        .args(["add", "artifact", "body", "--type", "doc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'doc'"));
}
