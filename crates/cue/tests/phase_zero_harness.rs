mod helpers;

use std::process::Command;

#[test]
fn test_env_provides_central_store_and_origin_repo() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    assert!(env.cue_home().is_dir());
    assert!(env.cue_home().starts_with(env.root()));

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(env.root())
        .output()?;

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        helpers::TEST_ORIGIN_URL
    );

    Ok(())
}
