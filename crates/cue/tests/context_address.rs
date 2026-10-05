//! A context selector may be a bare slug or a canonical address.
//!
//! `cue status` prints a canonical `<org>/<repo>/<slug>` address so a context
//! has one copyable identity. These tests hold the other half of that
//! contract: the address it prints can be handed straight back to any
//! context-accepting surface. The address form is authoritative for
//! destination resolution, so it names any scope of the selected store,
//! matching the cwd scope or not; a bare slug keeps the cwd scope.

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

/// A context under a foreign scope, planted directly in the store: no
/// checkout of that repository exists, which is precisely the situation
/// `-C` cannot serve.
fn plant_foreign_context(env: &helpers::TestEnv, scope: &str, slug: &str) -> anyhow::Result<()> {
    let context_dir = env.cue_store().join(scope).join(slug);
    std::fs::create_dir_all(context_dir.join("spec"))?;
    std::fs::write(
        context_dir.join("context.md"),
        "---\ntitle: Foreign\nkind: work\ncreated_at: 1\n---\n",
    )?;
    std::fs::write(context_dir.join("spec/index.md"), "Foreign spec\n")?;
    Ok(())
}

#[test]
fn a_cross_scope_address_lists_the_addressed_context() -> anyhow::Result<()> {
    let env = env_with_context();
    plant_foreign_context(&env, "other/repo", "guest")?;

    env.command()
        .args(["list", "--context", "other/repo/guest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("spec/index.md"));

    Ok(())
}

#[test]
fn a_cross_scope_address_reports_the_addressed_scope() -> anyhow::Result<()> {
    let env = env_with_context();
    plant_foreign_context(&env, "other/repo", "guest")?;

    env.command()
        .args(["status", "--context", "other/repo/guest", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"context\":\"guest\""))
        .stdout(predicate::str::contains("\"address\":\"other/repo/guest\""))
        .stdout(predicate::str::contains("\"scope\":\"other/repo\""));

    Ok(())
}

#[test]
fn a_cross_scope_address_renders_the_addressed_context() -> anyhow::Result<()> {
    let env = env_with_context();
    plant_foreign_context(&env, "other/repo", "guest")?;

    let stdout = env
        .command()
        .args(["render", "spec/index.md", "--context", "other/repo/guest"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let path = env.cue_store().join("other/repo/guest/spec/index.md");
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
fn a_cross_scope_address_writes_into_the_addressed_scope() -> anyhow::Result<()> {
    let env = env_with_context();
    plant_foreign_context(&env, "other/repo", "guest")?;

    env.command()
        .args([
            "add",
            "rollout",
            "Rollout steps",
            "--type",
            "plan",
            "--context",
            "other/repo/guest",
        ])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join("other/repo/guest/plan/rollout.md")
            .is_file(),
        "the addressed scope is authoritative for the destination"
    );
    assert!(
        !env.cue_store()
            .join("acme/widgets/guest/plan/rollout.md")
            .exists(),
        "a cross-scope write must not land in the cwd scope"
    );

    Ok(())
}

#[test]
fn a_cross_scope_address_logs_into_the_addressed_scope() -> anyhow::Result<()> {
    let env = env_with_context();
    plant_foreign_context(&env, "other/repo", "guest")?;

    env.command()
        .args([
            "log",
            "add",
            "--title",
            "Visited from another repository",
            "--context",
            "other/repo/guest",
        ])
        .assert()
        .success();

    env.command()
        .args(["log", "list", "--context", "other/repo/guest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Visited from another repository"));

    Ok(())
}

#[test]
fn a_missing_context_in_another_scope_names_the_address() {
    let env = env_with_context();

    env.command()
        .args(["status", "--context", "other/repo/absent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("other/repo/absent"));
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
