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
