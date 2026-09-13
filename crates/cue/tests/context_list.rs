mod helpers;

use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// A second repository scope, so whole-store breadth has more than one scope
/// to widen to. Only an origin remote is needed: the store scope is derived
/// from it, and listing reads no revision.
fn setup_scope_repo(path: &Path, origin: &str) {
    std::fs::create_dir_all(path).expect("Failed to create scope repo dir");
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .output()
        .expect("Failed to init scope repo");
    helpers::setup_origin(path, origin);
}

#[test]
fn context_list_json_reports_central_context_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["roadmap", "architecture"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
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
    // The referenced contexts must exist before they can be referenced, so
    // they are listed alongside the one under test.
    assert_eq!(contexts.as_array().map(Vec::len), Some(3));
    let context = contexts
        .as_array()
        .and_then(|contexts| {
            contexts
                .iter()
                .find(|context| context["context"] == "release")
        })
        .expect("release context should be listed");

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

/// `--scope repo` is the default, so naming it explicitly must not change
/// what is listed, and must not widen to contexts in another scope.
#[test]
fn context_list_scope_repo_matches_the_default_view() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    for slug in ["zeta", "alpha"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "repo"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

/// `--scope store` widens to every repository scope. A bare slug no longer
/// identifies a context once the query spans scopes, so each line is the
/// canonical `<org>/<repo>/<slug>` address, ordered by that address.
#[test]
fn context_list_scope_store_lists_every_scope_as_canonical_addresses() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    let abacus = env.root().join("abacus-repo");
    setup_scope_repo(&abacus, "https://github.com/abacus/tools.git");

    for slug in ["zeta", "alpha"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&abacus)
        .args(["context", "create", "build"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout(
            "abacus/tools/build\nacme/widgets/alpha\nacme/widgets/zeta\nother/project/roadmap\n",
        );
}

/// JSON already carries `scope` per entry, so widening adds entries rather
/// than changing their shape.
#[test]
fn context_list_scope_store_json_reports_every_scope() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    let output = env
        .command()
        .args(["context", "list", "--scope", "store", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    let contexts = contexts.as_array().expect("contexts should be an array");

    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0]["scope"], "acme/widgets");
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[1]["scope"], "other/project");
    assert_eq!(contexts[1]["context"], "roadmap");

    Ok(())
}

/// Scope is a closed vocabulary of `repo|store`; anything else is rejected
/// rather than silently widened or narrowed.
#[test]
fn context_list_rejects_an_unknown_scope_value() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in ["", "all", "Repo", "global", "repository"] {
        env.command()
            .args(["context", "list", "--scope", invalid])
            .assert()
            .failure()
            .stderr(predicate::str::contains("repo"))
            .stderr(predicate::str::contains("store"));
    }
}

/// The whole-store view resolves no repository scope, so it works from
/// anywhere, including outside a Git repository. The default view still
/// requires an origin remote.
#[test]
fn context_list_scope_store_needs_no_repository_scope() {
    let env = helpers::TestEnv::new();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("other/project/roadmap\n");

    env.command().args(["context", "list"]).assert().failure();
}

/// Only `<org>/<repo>` directories are scopes. Store-internal state lives in
/// dot-directories under the store root, so the whole-store walk must skip
/// them rather than mistaking their contents for contexts.
#[test]
fn context_list_scope_store_skips_store_internal_state() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success();
    assert!(
        env.cue_store()
            .join(".state/pins/acme/widgets/release")
            .is_file()
    );

    // A decoy shaped exactly like a context, planted inside store-internal
    // state: only skipping dot-directories keeps it out of the listing.
    let decoy = env.cue_store().join(".state/decoy/scope/imposter");
    std::fs::create_dir_all(&decoy)?;
    std::fs::write(
        decoy.join("context.md"),
        "---\nkind: work\ncreated_at: 1\n---\n",
    )?;

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    Ok(())
}

/// A scope directory holding no context is not an error, and contributes no
/// lines to the whole-store view.
#[test]
fn context_list_scope_store_skips_scopes_without_contexts() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    std::fs::create_dir_all(env.cue_store().join("empty/scope"))?;
    std::fs::write(env.cue_store().join("acme/stray"), "")?;

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    Ok(())
}
