mod helpers;

use serde_json::Value;

#[test]
fn status_json_describes_an_explicit_central_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
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
    // The canonical address is what identifies the context outside this
    // repository, so it is emitted rather than left to be concatenated.
    assert_eq!(status["address"], "acme/widgets/release");

    Ok(())
}

/// The human view reports the same canonical address, so a value copied from
/// a terminal identifies the context without also knowing the repository.
#[test]
fn status_prints_the_canonical_address() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args(["status", "--context", "release"])
        .assert()
        .success()
        .stdout(predicates::str::contains("address: acme/widgets/release"));
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
    // There is no context to address, so the field is present and null
    // rather than absent or a bare scope.
    assert!(status["address"].is_null());

    Ok(())
}
