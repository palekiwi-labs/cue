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

#[test]
fn multiple_entries_keep_argument_order() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for (artifact_type, content) in [("spec", "Release scope"), ("plan", "Release steps")] {
        env.command()
            .args([
                "add",
                "index",
                content,
                "--type",
                artifact_type,
                "--context",
                "release",
            ])
            .assert()
            .success();
    }

    let stdout = env
        .command()
        .args([
            "context",
            "render",
            "spec/index.md",
            "plan/index.md",
            "--context",
            "release",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let context_dir = env.cue_store().join("acme/widgets/release");
    let spec_path = context_dir.join("spec/index.md");
    let plan_path = context_dir.join("plan/index.md");
    let spec = std::fs::read_to_string(&spec_path)?;
    let plan = std::fs::read_to_string(&plan_path)?;
    assert_eq!(
        String::from_utf8(stdout)?,
        format!(
            "<artifact path=\"{}\">\n{spec}\n</artifact>\n\n<artifact path=\"{}\">\n{plan}\n</artifact>\n\n",
            spec_path.display(),
            plan_path.display()
        )
    );

    Ok(())
}

#[test]
fn missing_entry_is_skipped_silently() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "context",
            "render",
            "spec/missing.md",
            "--context",
            "release",
        ])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn repeated_entry_is_emitted_once() -> anyhow::Result<()> {
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
        .args([
            "context",
            "render",
            "spec/index.md",
            "spec/index.md",
            "--context",
            "release",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(stdout)?.matches("<artifact ").count(), 1);

    Ok(())
}
