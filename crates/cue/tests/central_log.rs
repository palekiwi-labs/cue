mod helpers;

#[test]
fn log_add_writes_one_entry_file_in_the_central_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command().arg("init").assert().success();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args([
            "log",
            "add",
            "--task",
            "release",
            "--title",
            "Validated release",
            "--found",
            "Smoke test passed",
        ])
        .assert()
        .success();

    let log_dir = env.cue_home().join("acme/widgets/release/log");
    let entries = std::fs::read_dir(&log_dir)?.collect::<Result<Vec<_>, _>>()?;

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]
            .path()
            .extension()
            .and_then(|value| value.to_str()),
        Some("md")
    );
    let content = std::fs::read_to_string(entries[0].path())?;
    assert!(content.contains("Validated release"));
    assert!(content.contains("Smoke test passed"));
    assert!(!env.root().join(".cue").exists());

    Ok(())
}
