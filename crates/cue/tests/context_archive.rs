mod helpers;

use serde_json::Value;
use std::path::PathBuf;

/// The store directory of a context in the fixture repository's scope.
fn context_dir(env: &helpers::TestEnv, slug: &str) -> PathBuf {
    env.cue_store().join("acme/widgets").join(slug)
}

fn create(env: &helpers::TestEnv, args: &[&str]) {
    env.command()
        .args(["context", "create"])
        .args(args)
        .assert()
        .success();
}

/// Mark a context archived by hand, independently of the archive command.
fn plant_archived(env: &helpers::TestEnv, slug: &str, archived_at: &str) {
    let path = context_dir(env, slug).join("context.md");
    let text = std::fs::read_to_string(&path).expect("read context.md");
    let text = text.replacen(
        "\n---\n",
        &format!("\narchived_at: {archived_at}\n---\n"),
        1,
    );
    std::fs::write(path, text).expect("write context.md");
}

fn stdout(env: &helpers::TestEnv, args: &[&str]) -> String {
    let output = env.command().args(args).assert().success();
    String::from_utf8(output.get_output().stdout.clone()).expect("utf-8 stdout")
}

fn json(env: &helpers::TestEnv, args: &[&str]) -> Value {
    serde_json::from_str(&stdout(env, args)).expect("JSON stdout")
}

/// Two live contexts and one archived, all pinned.
fn fixture() -> helpers::TestEnv {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["program"]);
    create(&env, &["child", "--parent", "acme/widgets/program"]);
    create(&env, &["live"]);
    plant_archived(&env, "program", "1800000000");
    for slug in ["program", "child", "live"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }
    env
}

#[test]
fn archived_contexts_are_hidden_from_every_listing_by_default() {
    let env = fixture();
    let live = "acme/widgets/child\nacme/widgets/live\n";
    assert_eq!(stdout(&env, &["context", "list"]), live);
    assert_eq!(stdout(&env, &["context", "list", "--pinned"]), live);
    assert_eq!(stdout(&env, &["context", "list", "--scope", "store"]), live);
}

#[test]
fn archived_flag_includes_archived_contexts() {
    let env = fixture();
    let all = "acme/widgets/child\nacme/widgets/live\nacme/widgets/program\n";
    assert_eq!(stdout(&env, &["context", "list", "--archived"]), all);
    assert_eq!(
        stdout(&env, &["context", "list", "--pinned", "--archived"]),
        all
    );
    assert_eq!(
        stdout(
            &env,
            &[
                "context",
                "list",
                "--archived",
                "--filter",
                "archived_at=1800000000"
            ]
        ),
        "acme/widgets/program\n"
    );
}

#[test]
fn list_json_reports_archived_at() {
    let env = fixture();
    let rows = json(&env, &["context", "list", "--json", "--archived"]);
    let archived_at = |slug: &str| {
        rows.as_array()
            .unwrap()
            .iter()
            .find(|row| row["context"] == slug)
            .unwrap()["archived_at"]
            .clone()
    };
    assert_eq!(archived_at("program"), Value::from(1800000000u64));
    assert_eq!(archived_at("live"), Value::Null);
}

#[test]
fn tree_omits_archived_parents_and_promotes_their_children() {
    let env = fixture();
    for extra in [&[][..], &["--pinned"][..]] {
        let mut args = vec!["context", "tree", "--json"];
        args.extend_from_slice(extra);
        let tree = json(&env, &args);
        let roots: Vec<&str> = tree["roots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["context"].as_str().unwrap())
            .collect();
        assert_eq!(roots, ["child", "live"], "args: {args:?}");
    }

    let tree = json(&env, &["context", "tree", "--json", "--archived"]);
    let roots: Vec<&str> = tree["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["context"].as_str().unwrap())
        .collect();
    assert_eq!(roots, ["live", "program"]);
}

#[test]
fn malformed_archived_at_fails_like_malformed_created_at() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["broken"]);
    plant_archived(&env, "broken", "yesterday");
    env.command()
        .args(["context", "list"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid context metadata"));
}
