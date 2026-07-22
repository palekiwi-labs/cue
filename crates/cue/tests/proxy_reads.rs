mod helpers;

use std::fs;
use std::path::Path;

/// Build a minimal valid cue store with a spec artifact and a context.json.
fn make_real_store_with_artifact(parent: &Path) -> std::path::PathBuf {
    let store = parent.join(".cue");
    let spec_dir = store.join("master/spec");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(spec_dir.join("index.md"), "# Spec content").unwrap();

    // context.json pointing at the spec artifact.
    let context_json = store.join("master/context.json");
    fs::write(
        &context_json,
        r#"{"default":{"artifacts":["./spec/index.md"]}}"#,
    )
    .unwrap();

    store
}

/// Set up the worktree directory as its own git repo (so cue can resolve git
/// root to the worktree, not the parent) and run `cue link` to create the
/// proxy `.cue/`.
fn setup_proxy(env: &helpers::TestEnv, worktree: &Path, real_store: &Path, task: Option<&str>) {
    helpers::setup_git_repo(worktree);

    let mut cmd = env.command();
    cmd.current_dir(worktree)
        .arg("link")
        .arg(real_store.to_str().unwrap());

    if let Some(slug) = task {
        cmd.arg("--task").arg(slug);
    }

    cmd.assert().success();
}

// ---------------------------------------------------------------------------
// cue list --json: paths must be absolute
// ---------------------------------------------------------------------------

#[test]
fn list_json_in_proxy_emits_absolute_paths() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store_with_artifact(env.root());

    let worktree = env.root().join("worktrees/agent-list");
    fs::create_dir_all(&worktree)?;
    setup_proxy(&env, &worktree, &real_store, None);

    let output = env
        .command()
        .current_dir(&worktree)
        .args(["list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let artifacts = json.as_array().expect("expected JSON array");

    assert!(!artifacts.is_empty(), "expected at least one artifact");
    for entry in artifacts {
        let path = entry["path"].as_str().expect("path field must be a string");
        assert!(path.starts_with('/'), "path must be absolute, got: {path}");
        assert!(
            path.contains("master/"),
            "path must contain scope slug, got: {path}"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// cue list (human output): paths must be absolute
// ---------------------------------------------------------------------------

#[test]
fn list_human_in_proxy_emits_absolute_paths() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store_with_artifact(env.root());

    let worktree = env.root().join("worktrees/agent-list-human");
    fs::create_dir_all(&worktree)?;
    setup_proxy(&env, &worktree, &real_store, None);

    let output = env
        .command()
        .current_dir(&worktree)
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output)?;
    for line in text.lines() {
        assert!(line.starts_with('/'), "path must be absolute, got: {line}");
        assert!(
            line.contains("master/"),
            "path must contain scope slug, got: {line}"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// cue context render: artifact path= attribute must be store-relative
// ---------------------------------------------------------------------------

#[test]
fn context_render_in_proxy_emits_store_relative_paths() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let real_store = make_real_store_with_artifact(env.root());

    let worktree = env.root().join("worktrees/agent-render");
    fs::create_dir_all(&worktree)?;
    setup_proxy(&env, &worktree, &real_store, None);

    let output = env
        .command()
        .current_dir(&worktree)
        .args(["context", "render"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output)?;

    // The rendered output must contain a store-relative path attribute.
    assert!(
        text.contains("path=\"master/spec/index.md\""),
        "expected store-relative path in render output, got:\n{text}"
    );
    // Must NOT contain an absolute host path.
    assert!(
        !text.contains("path=\"/"),
        "must not emit absolute path in render output, got:\n{text}"
    );

    Ok(())
}
