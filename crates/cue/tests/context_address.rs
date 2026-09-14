//! A context selector may be a bare slug or a canonical address.
//!
//! `cue status` prints a canonical `<org>/<repo>/<slug>` address so a context
//! has one copyable identity. These tests hold the other half of that
//! contract: the address it prints can be handed straight back to any
//! context-accepting surface. The address must name the current repository
//! scope, because the scope a write lands in is derived from the working
//! directory and nothing else selects it.

mod helpers;

use predicates::prelude::*;
use std::process::Command;

const CONTEXT: &str = "release";
const ADDRESS: &str = "acme/widgets/release";

/// A fixture repository with one context holding a spec artifact.
fn env_with_context() -> helpers::TestEnv {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", CONTEXT])
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
            CONTEXT,
        ])
        .assert()
        .success();
    env
}

#[test]
fn status_accepts_a_canonical_address() {
    let env = env_with_context();

    env.command()
        .args(["status", "--context", ADDRESS, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\":\"release\""))
        .stdout(predicate::str::contains(
            "\"address\":\"acme/widgets/release\"",
        ));
}

#[test]
fn render_accepts_a_canonical_address() -> anyhow::Result<()> {
    let env = env_with_context();

    let stdout = env
        .command()
        .args(["render", "spec/index.md", "--context", ADDRESS])
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
fn list_accepts_a_canonical_address() {
    let env = env_with_context();

    env.command()
        .args(["list", "--context", ADDRESS])
        .assert()
        .success()
        .stdout(predicate::str::contains("spec/index.md"));
}

#[test]
fn add_accepts_a_canonical_address() {
    let env = env_with_context();

    env.command()
        .args([
            "add",
            "rollout",
            "Rollout steps",
            "--type",
            "plan",
            "--context",
            ADDRESS,
        ])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join("acme/widgets/release/plan/rollout.md")
            .is_file(),
        "canonical address should resolve to the same destination as the slug"
    );
}

#[test]
fn log_accepts_a_canonical_address() {
    let env = env_with_context();

    env.command()
        .args([
            "log",
            "add",
            "--title",
            "Cut the release",
            "--context",
            ADDRESS,
        ])
        .assert()
        .success();

    env.command()
        .args(["log", "list", "--context", ADDRESS])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cut the release"));
}

#[test]
fn cue_context_environment_accepts_a_canonical_address() {
    let env = env_with_context();

    env.command()
        .env("CUE_CONTEXT", ADDRESS)
        .args(["status", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\":\"release\""));
}

#[test]
fn branch_configuration_accepts_a_canonical_address() {
    let env = env_with_context();
    let output = Command::new("git")
        .args(["config", "branch.main.cue-context", ADDRESS])
        .current_dir(env.root())
        .output()
        .expect("Failed to set branch context");
    assert!(output.status.success());

    env.command()
        .args(["status", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\":\"release\""));
}

#[test]
fn an_address_in_another_scope_is_rejected() {
    let env = env_with_context();

    env.command()
        .args(["status", "--context", "other/repo/release"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("other/repo"))
        .stderr(predicate::str::contains("acme/widgets"));
}

#[test]
fn an_artifact_address_is_rejected_as_a_context() {
    let env = env_with_context();

    env.command()
        .args(["status", "--context", "acme/widgets/release/spec/index.md"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("<org>/<repo>/<slug>"));
}

#[test]
fn a_partial_address_is_rejected() {
    let env = env_with_context();

    for value in ["widgets/release", "~/cue/acme/widgets/release"] {
        env.command()
            .args(["status", "--context", value])
            .assert()
            .failure()
            .stderr(predicate::str::contains("<org>/<repo>/<slug>"));
    }
}

#[test]
fn an_unsafe_address_segment_is_rejected() {
    let env = env_with_context();

    for value in [
        "acme/../release",
        "/acme/widgets/release",
        "acme/widgets/ ",
        "acme/widgets/rel\nease",
    ] {
        env.command()
            .args(["status", "--context", value])
            .assert()
            .failure();
    }
}
