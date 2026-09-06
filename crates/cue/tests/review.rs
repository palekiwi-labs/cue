mod helpers;

use helpers::TestEnv;
use predicates::prelude::*;

#[test]
fn review_help_lists_every_pipeline_stage() {
    let env = TestEnv::new();

    env.command()
        .args(["review", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("submit"))
        .stdout(predicate::str::contains("verdict"))
        .stdout(predicate::str::contains("finalize"))
        .stdout(predicate::str::contains("schema"));
}

#[test]
fn review_without_a_subcommand_is_an_error() {
    let env = TestEnv::new();

    env.command().arg("review").assert().failure();
}

#[test]
fn submit_requires_a_run_and_a_model() {
    let env = TestEnv::new();

    env.command()
        .args(["review", "submit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--run"))
        .stderr(predicate::str::contains("--model"));
}

#[test]
fn verdict_requires_a_run_and_a_model() {
    let env = TestEnv::new();

    env.command()
        .args(["review", "verdict"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--run"))
        .stderr(predicate::str::contains("--model"));
}

#[test]
fn submit_rejects_a_model_id_with_no_usable_slug() {
    let env = TestEnv::new();

    env.command()
        .args([
            "review",
            "submit",
            "--run",
            "1788586933-cfff1d9",
            "--model",
            "///",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty slug"));
}

#[test]
fn finalize_requires_a_run() {
    let env = TestEnv::new();

    env.command()
        .args(["review", "finalize"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--run"));
}
