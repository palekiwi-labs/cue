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

fn read_record(env: &helpers::TestEnv, slug: &str) -> String {
    std::fs::read_to_string(context_dir(env, slug).join("context.md")).expect("read context.md")
}

#[test]
fn archive_appends_archived_at_and_unarchive_restores_the_record() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["program"]);
    create(
        &env,
        &[
            "release",
            "--title",
            "Release: cue",
            "--ref",
            "acme/widgets/program",
        ],
    );
    let path = context_dir(&env, "release").join("context.md");
    let mode = std::fs::metadata(&path).unwrap().permissions();
    let before = read_record(&env, "release");

    env.command()
        .args(["context", "archive", "release"])
        .assert()
        .success()
        .stdout("archived acme/widgets/release\n");
    let archived = read_record(&env, "release");
    let (head, tail) = archived
        .rsplit_once("archived_at: ")
        .expect("archived_at appended");
    let (stamp, rest) = tail.split_once('\n').unwrap();
    assert!(stamp.parse::<u64>().unwrap() > 1_700_000_000);
    assert_eq!(rest, "---\n");
    assert_eq!(format!("{head}{rest}"), before);

    env.command()
        .args(["context", "unarchive", "release"])
        .assert()
        .success()
        .stdout("unarchived acme/widgets/release\n");
    assert_eq!(read_record(&env, "release"), before);
    assert_eq!(std::fs::metadata(&path).unwrap().permissions(), mode);
}

#[test]
fn archiving_twice_keeps_the_first_stamp_and_unarchiving_twice_is_harmless() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["release"]);
    plant_archived(&env, "release", "1800000000");
    let archived = read_record(&env, "release");

    env.command()
        .args(["context", "archive", "release"])
        .assert()
        .success();
    assert_eq!(read_record(&env, "release"), archived);

    create(&env, &["live"]);
    let live = read_record(&env, "live");
    env.command()
        .args(["context", "unarchive", "live"])
        .assert()
        .success();
    assert_eq!(read_record(&env, "live"), live);
}

#[test]
fn any_kind_can_be_archived() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["codebase", "--kind", "reference"]);
    env.command()
        .args(["context", "archive", "codebase"])
        .assert()
        .success();
    assert!(read_record(&env, "codebase").contains("\narchived_at: "));
}

#[test]
fn each_context_is_processed_and_any_failure_fails_the_command() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["one"]);
    create(&env, &["two"]);
    env.command()
        .args(["context", "archive", "one", "missing", "acme/widgets/two"])
        .assert()
        .failure()
        .stdout("archived acme/widgets/one\narchived acme/widgets/two\n")
        .stderr(predicates::str::contains("acme/widgets/missing"));
    assert!(read_record(&env, "one").contains("archived_at"));
    assert!(read_record(&env, "two").contains("archived_at"));
}

#[test]
fn an_archived_context_still_resolves_by_address() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["program"]);
    env.command()
        .args(["context", "archive", "program"])
        .assert()
        .success();

    create(&env, &["child", "--parent", "acme/widgets/program"]);
    env.command()
        .args(["status", "--context", "program"])
        .assert()
        .success();
    env.command()
        .args([
            "log",
            "add",
            "--context",
            "program",
            "--title",
            "still here",
        ])
        .assert()
        .success();
    env.command()
        .args(["add", "index.md", "why", "--context", "program"])
        .assert()
        .success();
    env.command()
        .args(["render", "spec/index.md", "--context", "program"])
        .assert()
        .success();
}

/// Replace the `archived_at` line planted by `plant_archived` with `line`.
fn plant_line(env: &helpers::TestEnv, slug: &str, line: &str) {
    plant_archived(env, slug, "0");
    let path = context_dir(env, slug).join("context.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, text.replacen("archived_at: 0", line, 1)).unwrap();
}

#[test]
fn archiving_a_null_stamp_replaces_it() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["release"]);
    plant_line(&env, "release", "archived_at: null");
    env.command()
        .args(["context", "archive", "release"])
        .assert()
        .success();
    let record = read_record(&env, "release");
    assert!(!record.contains("null"));
    assert_eq!(record.matches("archived_at: ").count(), 1);
    assert_eq!(stdout(&env, &["context", "list"]), "");
}

#[test]
fn a_key_spelling_the_editor_cannot_edit_is_refused() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    create(&env, &["release"]);
    plant_line(&env, "release", "\"archived_at\": 1");
    let before = read_record(&env, "release");
    env.command()
        .args(["context", "unarchive", "release"])
        .assert()
        .failure();
    assert_eq!(read_record(&env, "release"), before);
}

#[test]
fn unarchive_accepts_an_address_and_tree_root_respects_archival() {
    let env = fixture();
    env.command()
        .args(["context", "tree", "--root", "program"])
        .assert()
        .failure();
    env.command()
        .args(["context", "tree", "--root", "program", "--archived"])
        .assert()
        .success();
    env.command()
        .args(["context", "unarchive", "acme/widgets/program"])
        .assert()
        .success()
        .stdout("unarchived acme/widgets/program\n");
    env.command()
        .args(["context", "tree", "--root", "program"])
        .assert()
        .success();
}
