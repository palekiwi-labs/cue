mod helpers;

use predicates::prelude::*;
use std::fs;

#[test]
fn test_config_show_json() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Create a project-specific config
    fs::write(
        env.root().join("cue.json"),
        r#"{"dir_name": ".custom-mem"}"#,
    )?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-branch")
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""dir_name": ".custom-mem""#))
        .stdout(predicate::str::contains(r#""branch_name": "test-branch""#));

    Ok(())
}
