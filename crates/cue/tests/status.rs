mod helpers;

use serde_json::Value;

#[test]
fn status_json_describes_an_explicit_central_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args([
            "context",
            "create",
            "release",
            "--title",
            "Release cue",
            "--kind",
            "coord",
            "--mode",
            "build",
            "--parent",
            "acme/widgets/roadmap",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["status", "--context", "release", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: Value = serde_json::from_slice(&output)?;

    assert_eq!(status["context"], "release");
    assert_eq!(status["title"], "Release cue");
    assert_eq!(status["kind"], "coord");
    assert_eq!(status["mode"], "build");
    assert_eq!(status["parent"], "acme/widgets/roadmap");
    assert_eq!(status["store"], env.cue_store().to_str().unwrap());
    assert_eq!(status["scope"], "acme/widgets");

    Ok(())
}

#[test]
fn status_json_reports_an_unset_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    let output = env
        .command()
        .args(["status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: Value = serde_json::from_slice(&output)?;

    assert!(status["context"].is_null());
    assert_eq!(status["store"], env.cue_store().to_str().unwrap());
    assert_eq!(status["scope"], "acme/widgets");

    Ok(())
}
