mod helpers;

use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;

/// Plant a context directly in the store. `context create` validates that a
/// parent exists, so illegal shapes such as cycles can only be planted.
fn plant(store: &Path, scope: &str, slug: &str, parent: Option<&str>) {
    let dir = store.join(scope).join(slug);
    std::fs::create_dir_all(&dir).expect("Failed to create context dir");
    let parent = parent
        .map(|parent| format!("parent: {parent}\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.join("context.md"),
        format!("---\nkind: work\ncreated_at: 1\n{parent}---\n"),
    )
    .expect("Failed to write context.md");
}

fn plant_log(store: &Path, scope: &str, slug: &str, timestamp: u64) {
    let log_dir = store.join(scope).join(slug).join("log");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log dir");
    std::fs::write(
        log_dir.join(format!("{timestamp:020}.json")),
        format!(
            "{{\"timestamp\":{timestamp},\"title\":\"t\",\"trace\":null,\
             \"found\":[],\"decided\":[],\"open\":[]}}\n"
        ),
    )
    .expect("Failed to write log entry");
}

fn tree_json(env: &helpers::TestEnv, args: &[&str]) -> anyhow::Result<Value> {
    let output = env
        .command()
        .args(["context", "tree", "--json"])
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    Ok(serde_json::from_slice(&output)?)
}

/// Reduce a tree to `address(children...)` so shape assertions stay readable.
fn shape(nodes: &Value) -> String {
    nodes
        .as_array()
        .expect("nodes should be an array")
        .iter()
        .map(|node| {
            let address = format!(
                "{}/{}",
                node["scope"].as_str().unwrap_or_default(),
                node["context"].as_str().unwrap_or_default()
            );
            let children = shape(&node["children"]);
            if children.is_empty() {
                address
            } else {
                format!("{address}({children})")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const S: &str = "acme/widgets";

#[test]
fn tree_nests_by_parent_and_roots_unresolved_and_self_parents() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));
    plant(store, S, "grand", Some("acme/widgets/child"));
    plant(store, S, "orphan", Some("acme/widgets/gone"));
    plant(store, S, "selfish", Some("acme/widgets/selfish"));

    let tree = tree_json(&env, &[])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/orphan \
         acme/widgets/program(acme/widgets/child(acme/widgets/grand)) \
         acme/widgets/selfish"
    );
    assert_eq!(tree["diagnostics"], serde_json::json!([]));
    // A node is a context list row plus children.
    let program = &tree["roots"][1];
    assert_eq!(program["kind"], "work");
    assert_eq!(program["pinned"], false);
    Ok(())
}

#[test]
fn tree_reports_cycles_and_emits_every_member_once_as_a_root() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "a", Some("acme/widgets/b"));
    plant(store, S, "b", Some("acme/widgets/a"));
    plant(store, S, "leaf", Some("acme/widgets/a"));

    let tree = tree_json(&env, &[])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/a(acme/widgets/leaf) acme/widgets/b"
    );
    let diagnostics = tree["diagnostics"].as_array().expect("array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["kind"], "cycle");
    assert_eq!(
        diagnostics[0]["contexts"],
        serde_json::json!(["acme/widgets/a", "acme/widgets/b"])
    );
    Ok(())
}

#[test]
fn tree_orders_by_subtree_recency() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "alpha", None);
    plant(store, S, "beta", None);
    plant(store, S, "beta-old", Some("acme/widgets/beta"));
    plant(store, S, "beta-new", Some("acme/widgets/beta"));
    plant(store, S, "quiet", None);
    plant_log(store, S, "alpha", 2_000_000_000_000_000_000);
    plant_log(store, S, "beta-old", 1_000_000_000_000_000_000);
    plant_log(store, S, "beta-new", 3_000_000_000_000_000_000);

    let tree = tree_json(&env, &["--sort", "recency"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/beta(acme/widgets/beta-new acme/widgets/beta-old) \
         acme/widgets/alpha acme/widgets/quiet"
    );
    Ok(())
}

#[test]
fn tree_pinned_narrows_before_building() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));
    plant(store, S, "other", None);
    for slug in ["child", "other"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    let tree = tree_json(&env, &["--pinned"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/child acme/widgets/other"
    );
    assert_eq!(tree["diagnostics"], serde_json::json!([]));
    Ok(())
}

#[test]
fn tree_filter_narrows_before_building() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));

    let tree = tree_json(&env, &["--filter", "parent~=program"])?;
    assert_eq!(shape(&tree["roots"]), "acme/widgets/child");
    Ok(())
}

#[test]
fn tree_links_across_scopes_at_store_breadth() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, "acme/program", "coord", None);
    plant(store, S, "work", Some("acme/program/coord"));

    let tree = tree_json(&env, &["--scope", "store"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/program/coord(acme/widgets/work)"
    );
    // At repo breadth the foreign parent is outside the set: a plain root.
    let tree = tree_json(&env, &[])?;
    assert_eq!(shape(&tree["roots"]), "acme/widgets/work");
    Ok(())
}

#[test]
fn tree_root_selects_a_subtree() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));
    plant(store, S, "grand", Some("acme/widgets/child"));
    plant(store, S, "other", None);

    let tree = tree_json(&env, &["--root", "child"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/child(acme/widgets/grand)"
    );
    let tree = tree_json(&env, &["--root", "acme/widgets/program"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/program(acme/widgets/child(acme/widgets/grand))"
    );
    Ok(())
}

#[test]
fn tree_root_inside_pinned_keeps_only_pinned_descendants() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));
    plant(store, S, "skipped", Some("acme/widgets/program"));
    for slug in ["program", "child"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    let tree = tree_json(&env, &["--pinned", "--root", "program"])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/program(acme/widgets/child)"
    );
    Ok(())
}

#[test]
fn tree_root_excluded_by_filter_is_an_error() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "child", Some("acme/widgets/program"));
    env.command()
        .args(["context", "pin", "child"])
        .assert()
        .success();

    env.command()
        .args(["context", "tree", "--pinned", "--root", "program"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("acme/widgets/program"));
}

#[test]
fn tree_plain_output_draws_boxes() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    plant(store, S, "a", Some("acme/widgets/program"));
    plant(store, S, "a1", Some("acme/widgets/a"));
    plant(store, S, "b", Some("acme/widgets/program"));

    env.command()
        .args(["context", "tree"])
        .assert()
        .success()
        .stdout(
            "acme/widgets/program\n\
             ├─ acme/widgets/a\n\
             │  └─ acme/widgets/a1\n\
             └─ acme/widgets/b\n",
        );
}

#[test]
fn tree_plain_output_reports_cycles_on_stderr() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "a", Some("acme/widgets/b"));
    plant(store, S, "b", Some("acme/widgets/a"));

    env.command()
        .args(["context", "tree"])
        .assert()
        .success()
        .stdout("acme/widgets/a\nacme/widgets/b\n")
        .stderr(predicate::str::contains("cycle"));
}

#[test]
fn tree_breaks_long_and_multiple_cycles_reached_through_a_tail() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    // `a-tail` sorts first, so the cycle is entered through it.
    plant(store, S, "a-tail", Some("acme/widgets/b"));
    plant(store, S, "b", Some("acme/widgets/c"));
    plant(store, S, "c", Some("acme/widgets/d"));
    plant(store, S, "d", Some("acme/widgets/b"));
    plant(store, S, "e", Some("acme/widgets/f"));
    plant(store, S, "f", Some("acme/widgets/e"));

    let tree = tree_json(&env, &[])?;
    assert_eq!(
        shape(&tree["roots"]),
        "acme/widgets/b(acme/widgets/a-tail) acme/widgets/c acme/widgets/d \
         acme/widgets/e acme/widgets/f"
    );
    assert_eq!(
        tree["diagnostics"],
        serde_json::json!([
            {"kind": "cycle", "contexts": ["acme/widgets/b", "acme/widgets/c", "acme/widgets/d"]},
            {"kind": "cycle", "contexts": ["acme/widgets/e", "acme/widgets/f"]},
        ])
    );
    Ok(())
}

#[test]
fn tree_root_reports_only_cycles_containing_the_root() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "a", Some("acme/widgets/b"));
    plant(store, S, "b", Some("acme/widgets/a"));
    plant(store, S, "e", Some("acme/widgets/f"));
    plant(store, S, "f", Some("acme/widgets/e"));
    plant(store, S, "program", None);

    let tree = tree_json(&env, &["--root", "program"])?;
    assert_eq!(tree["diagnostics"], serde_json::json!([]));
    let tree = tree_json(&env, &["--root", "a"])?;
    assert_eq!(
        tree["diagnostics"],
        serde_json::json!([{"kind": "cycle", "contexts": ["acme/widgets/a", "acme/widgets/b"]}])
    );
    Ok(())
}

#[test]
fn tree_plain_output_reads_logs_only_for_recency() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let store = env.cue_store();
    plant(store, S, "program", None);
    // A file where the log directory belongs fails any log scan.
    std::fs::write(store.join(S).join("program").join("log"), "")
        .expect("Failed to plant log file");

    env.command()
        .args(["context", "tree"])
        .assert()
        .success()
        .stdout("acme/widgets/program\n");
    env.command()
        .args(["context", "tree", "--sort", "recency"])
        .assert()
        .failure();
}
