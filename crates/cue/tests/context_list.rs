mod helpers;

use serde_json::Value;

#[test]
fn context_list_json_reports_central_context_metadata() -> anyhow::Result<()> {
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
            "--description",
            "Coordinate the release",
            "--parent",
            "acme/widgets/roadmap",
            "--ref",
            "acme/widgets/architecture",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["context", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    let context = &contexts[0];

    assert_eq!(contexts.as_array().map(Vec::len), Some(1));
    assert_eq!(context["context"], "release");
    assert_eq!(context["scope"], "acme/widgets");
    assert_eq!(context["title"], "Release cue");
    assert_eq!(context["kind"], "coord");
    assert_eq!(context["mode"], "build");
    assert_eq!(context["description"], "Coordinate the release");
    assert!(context["created_at"].as_u64().is_some());
    assert_eq!(context["parent"], "acme/widgets/roadmap");
    assert_eq!(context["refs"][0], "acme/widgets/architecture");

    Ok(())
}

#[test]
fn context_list_succeeds_with_no_contexts_when_repository_scope_is_missing() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    assert!(!env.cue_store().join("acme/widgets").exists());

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["context", "list", "--json"])
        .assert()
        .success()
        .stdout("[]\n");

    Ok(())
}

#[test]
fn context_list_prints_only_contexts_in_slug_order() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "zeta"])
        .assert()
        .success();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    std::fs::create_dir(env.cue_store().join("acme/widgets/not-a-context"))?;

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");

    Ok(())
}
