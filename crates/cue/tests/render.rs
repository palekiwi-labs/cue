mod helpers;

use predicates::prelude::*;

#[test]
fn render_wraps_a_named_artifact_with_its_absolute_path() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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
        .args(["render", "spec/index.md", "--context", "release"])
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
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    env.command()
        .args(["render", "spec/missing.md", "--context", "release"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn repeated_entry_is_emitted_once() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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

#[test]
fn directory_entry_is_skipped() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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

    env.command()
        .args(["render", "spec", "--context", "release"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn render_requires_an_active_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["render", "context.md"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No context selected; pass --context <context>",
        ));
}

#[test]
fn render_rejects_a_nonexistent_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["render", "context.md", "--context", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Context does not exist: missing"));
}

#[test]
fn context_document_renders_as_an_entry() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    let stdout = env
        .command()
        .args(["render", "context.md", "--context", "release"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let path = env.cue_store().join("acme/widgets/release/context.md");
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
fn stdin_entries_render_at_the_marker_position() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
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

    let context_dir = env.cue_store().join("acme/widgets/release");
    let context_path = context_dir.join("context.md");
    let spec_path = context_dir.join("spec/index.md");
    let plan_path = context_dir.join("plan/index.md");
    let stdout = env
        .command()
        .args([
            "render",
            "context.md",
            "-",
            "plan/index.md",
            "--context",
            "release",
        ])
        .write_stdin(format!("\n  \n{}\n\t\n", spec_path.display()))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let context = std::fs::read_to_string(&context_path)?;
    let spec = std::fs::read_to_string(&spec_path)?;
    let plan = std::fs::read_to_string(&plan_path)?;
    assert_eq!(
        String::from_utf8(stdout)?,
        format!(
            "<artifact path=\"{}\">\n{context}\n</artifact>\n\n<artifact path=\"{}\">\n{spec}\n</artifact>\n\n<artifact path=\"{}\">\n{plan}\n</artifact>\n\n",
            context_path.display(),
            spec_path.display(),
            plan_path.display()
        )
    );

    Ok(())
}

#[test]
fn absolute_stdin_entries_do_not_require_an_active_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();

    let path = env.cue_store().join("acme/widgets/release/context.md");
    let content = std::fs::read_to_string(&path)?;
    env.command()
        .args(["render", "-"])
        .write_stdin(format!("{}\n", path.display()))
        .assert()
        .success()
        .stdout(format!(
            "<artifact path=\"{}\">\n{content}\n</artifact>\n\n",
            path.display()
        ));

    Ok(())
}

#[test]
fn filtered_list_output_pipes_into_render_after_named_anchors() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    for (name, content, artifact_type) in [
        ("index", "Release scope", "spec"),
        ("index", "Release steps", "plan"),
        ("ship", "Ship the release", "task"),
    ] {
        env.command()
            .args([
                "add",
                name,
                content,
                "--type",
                artifact_type,
                "--context",
                "release",
            ])
            .assert()
            .success();
    }
    env.command()
        .args([
            "add",
            "done",
            "Already done",
            "--type",
            "task",
            "--context",
            "release",
            "--frontmatter",
            "status=complete",
        ])
        .assert()
        .success();

    let selected_tasks = env
        .command()
        .args([
            "list",
            "--context",
            "release",
            "--type",
            "task",
            "--filter",
            "status!=complete",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let context_dir = env.cue_store().join("acme/widgets/release");
    let expected_paths = [
        context_dir.join("context.md"),
        context_dir.join("spec/index.md"),
        context_dir.join("plan/index.md"),
        context_dir.join("task/ship.md"),
    ];
    let mut expected = String::new();
    for path in expected_paths {
        let content = std::fs::read_to_string(&path)?;
        expected.push_str(&format!(
            "<artifact path=\"{}\">\n{content}\n</artifact>\n\n",
            path.display()
        ));
    }

    env.command()
        .args([
            "render",
            "context.md",
            "spec/index.md",
            "plan/index.md",
            "-",
            "--context",
            "release",
        ])
        .write_stdin(selected_tasks)
        .assert()
        .success()
        .stdout(expected);

    Ok(())
}
