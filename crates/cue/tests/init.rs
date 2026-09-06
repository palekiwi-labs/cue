mod helpers;

use predicates::prelude::*;

#[test]
fn test_init_creates_origin_scoped_central_store() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command().arg("init").assert().success();

    assert!(env.cue_home().join("acme/widgets").is_dir());
    assert!(
        !env.root().join(".cue").exists(),
        "init must not create a repository-local store"
    );
    assert!(
        !env.data_dir.join("projects.json").exists(),
        "init must not register checkout paths"
    );

    Ok(())
}

#[test]
fn test_init_not_a_git_repo() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();

    env.command()
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));

    Ok(())
}
