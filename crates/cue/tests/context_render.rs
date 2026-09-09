mod helpers;

#[test]
fn render_wraps_a_named_artifact_with_its_absolute_path() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args([
            "add",
            "index",
            "Release scope",
            "--type",
            "spec",
            "--context",
            "release",
        ])
        .assert()
        .success();

    let stdout = env
        .command()
        .args(["context", "render", "spec/index.md", "--context", "release"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let path = env.cue_store().join("acme/widgets/release/spec/index.md");
    let content = std::fs::read_to_string(&path)?;
    assert_eq!(
        String::from_utf8(stdout)?,
        format!(
            "<artifact path=\"{}\">\n{content}\n</artifact>\n\n",
            path.display()
        )
    );

    Ok(())
}
