mod helpers;

use helpers::TestEnv;
use serde_json::{Value, json};

fn fixture() -> TestEnv {
    let env = TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "work"])
        .assert()
        .success();
    env
}

fn events(env: &TestEnv) -> Vec<Value> {
    let output = env
        .command()
        .args(["activity", "list", "--context", "work", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn native_creation_records_actual_address_and_provenance() {
    let env = fixture();
    for kind in [
        "spec", "task", "plan", "note", "trace", "review", "bin", "tmp",
    ] {
        let output = env
            .command()
            .env_remove("CAST_HOST_NAME")
            .args([
                "add",
                "subject",
                "{}",
                "--type",
                kind,
                "--context",
                "work",
                "--session-id",
                "s1",
                "--harness",
                "other-harness",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let path = env.root().join(String::from_utf8(output).unwrap().trim());
        let address = path
            .strip_prefix(env.cue_store())
            .unwrap()
            .to_str()
            .unwrap();
        let entries = events(&env);
        let matches: Vec<_> = entries
            .iter()
            .filter(|e| e["details"]["address"] == address)
            .collect();
        assert_eq!(matches.len(), 1);
        let event = matches[0];
        assert_eq!(event["kind"], "artifact_created");
        assert_eq!(event["details"]["type"], kind);
        assert_eq!(event["harness"], "other-harness");
        assert_eq!(event["session_id"], "s1");
        assert!(event.get("host").is_none());
    }
    assert_eq!(events(&env).len(), 8);
    env.command()
        .args(["add", "subject", "{}", "--context", "work"])
        .assert()
        .failure();
    assert_eq!(events(&env).len(), 8);
}

#[test]
fn empty_provenance_is_omitted_and_host_defaults_from_environment() {
    let env = fixture();
    env.command()
        .env("CAST_HOST_NAME", "host-a")
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "git_commit",
            "--details",
            "{}",
            "--session-id",
            "",
            "--harness",
            "",
        ])
        .assert()
        .success();
    let entries = events(&env);
    assert_eq!(entries[0]["host"], "host-a");
    assert!(entries[0].get("session_id").is_none());
    assert!(entries[0].get("harness").is_none());
    env.command()
        .env("CAST_HOST_NAME", "host-a")
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "git_commit",
            "--details",
            "{}",
            "--host",
            "",
        ])
        .assert()
        .success();
    assert_eq!(
        events(&env)
            .iter()
            .filter(|e| e.get("host").is_none())
            .count(),
        1
    );
}

#[test]
fn recording_failure_does_not_fail_artifact_creation() {
    let env = fixture();
    std::fs::write(
        env.cue_store().join("acme/widgets/work/activity"),
        "obstruction",
    )
    .unwrap();
    env.command()
        .args(["add", "nested/artifact", "body", "--context", "work"])
        .assert()
        .success()
        .stderr(predicates::str::contains("activity recording failed"));
    assert!(
        env.cue_store()
            .join("acme/widgets/work/spec/nested/artifact.md")
            .is_file()
    );
    env.command()
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "git_commit",
            "--details",
            "{}",
        ])
        .assert()
        .failure();
    env.command()
        .args([
            "activity",
            "list",
            "--context",
            "work",
            "--limit",
            "0",
            "--json",
        ])
        .assert()
        .failure();
}

#[test]
fn canonical_destination_controls_native_attribution() {
    let env = fixture();
    let foreign = env.cue_store().join("other/repo/target");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("context.md"), "fixture").unwrap();
    for force in [false, true] {
        let mut command = env.command();
        command.env_remove("CAST_HOST_NAME").args([
            "add",
            "nested/file",
            "body",
            "--context",
            "other/repo/target",
        ]);
        if force {
            command.arg("--force");
        }
        command.assert().success();
    }
    assert!(events(&env).is_empty());
    let output = env
        .command()
        .args([
            "activity",
            "list",
            "--context",
            "other/repo/target",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: Vec<Value> = serde_json::from_slice(&output).unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(
            entry["details"]["address"],
            "other/repo/target/spec/nested/file.md"
        );
        for key in ["session_id", "harness", "host"] {
            assert!(entry.get(key).is_none());
        }
    }
}

#[test]
fn context_and_payload_validation_never_invents_a_destination() {
    let env = fixture();
    assert!(events(&env).is_empty());
    let args = ["activity", "add", "--kind", "git_commit", "--details", "{}"];
    env.command().args(args).assert().success().stdout("");
    assert!(events(&env).is_empty());
    env.command()
        .args(["activity", "list", "--json"])
        .assert()
        .failure();
    for selector in ["missing", "../invalid", "acme/widgets/missing"] {
        env.command()
            .args(args)
            .args(["--context", selector])
            .assert()
            .failure();
        env.command()
            .args(["activity", "list", "--context", selector, "--json"])
            .assert()
            .failure();
    }
    for details in ["[]", "null", "\"string\"", "{"] {
        env.command()
            .args([
                "activity",
                "add",
                "--context",
                "work",
                "--kind",
                "git_commit",
                "--details",
                details,
            ])
            .assert()
            .failure();
    }
    env.command()
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "Not a kind",
            "--details",
            "{}",
        ])
        .assert()
        .failure();
    assert!(events(&env).is_empty());
    env.command()
        .env("CUE_CONTEXT", "acme/widgets/work")
        .args(args)
        .assert()
        .success();
    assert_eq!(events(&env).len(), 1);
}

#[test]
fn readers_ignore_non_event_files_and_warn_for_bad_envelopes() {
    let env = fixture();
    let id = "00000000-0000-4000-8000-000000000001";
    let path = seed(&env, 1, id);
    let dir = path.parent().unwrap();
    for name in [
        "0000000000000000001-00000000-0000-4000-8000-000000000001.json",
        "00000000000000000002-00000000-0000-4000-8000-00000000000A.json",
        "00000000000000000001-00000000-0000-4000-8000-000000000001.sync-conflict-test.json",
        ".activity-staging-test",
        "unrelated.json",
    ] {
        std::fs::write(dir.join(name), "{").unwrap();
    }
    std::fs::create_dir(dir.join(format!("{:020}-{id}.json", 2))).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&path, dir.join(format!("{:020}-{id}.json", 3))).unwrap();
    for (timestamp, key, value) in [
        (4, "id", json!("00000000-0000-4000-8000-000000000002")),
        (5, "kind", json!("Not a kind")),
        (6, "details", json!([])),
        (7, "harness", json!(123)),
        ((1 << 53), "details", json!({})),
    ] {
        let path = seed(&env, timestamp, id);
        let mut event: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        event[key] = value;
        std::fs::write(path, event.to_string()).unwrap();
    }
    let output = env
        .command()
        .args(["activity", "list", "--context", "work", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(
        serde_json::from_slice::<Vec<Value>>(&output.stdout)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        String::from_utf8(output.stderr)
            .unwrap()
            .matches("Warning:")
            .count(),
        5
    );
    env.command()
        .args(["activity", "list", "--context", "work"])
        .assert()
        .success()
        .stdout(predicates::str::contains("git_commit"));
}

#[cfg(unix)]
#[test]
fn literal_backslashes_remain_distinct_from_path_separators() {
    let env = fixture();
    for filename in [r"a\b", "a/b"] {
        env.command()
            .args([
                "add",
                filename,
                "body",
                "--type",
                "note",
                "--context",
                "work",
            ])
            .assert()
            .success();
    }
    let entries = events(&env);
    let addresses: std::collections::HashSet<_> = entries
        .iter()
        .map(|event| event["details"]["address"].as_str().unwrap())
        .collect();
    assert_eq!(
        addresses,
        std::collections::HashSet::from([
            r"acme/widgets/work/note/a\b.md",
            "acme/widgets/work/note/a/b.md",
        ])
    );
    for address in addresses {
        assert!(env.cue_store().join(address).is_file());
    }
}

fn seed(env: &TestEnv, timestamp: u64, id: &str) -> std::path::PathBuf {
    let dir = env.cue_store().join("acme/widgets/work/activity");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{timestamp:020}-{id}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "id": id, "timestamp": timestamp, "kind": "git_commit", "details": {}
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn latest_valid_events_are_filtered_then_rendered_chronologically() {
    let env = fixture();
    let a = "00000000-0000-4000-8000-000000000001";
    let b = "00000000-0000-4000-8000-000000000002";
    let c = "00000000-0000-4000-8000-000000000003";
    seed(&env, 10, a);
    seed(&env, 20, c);
    seed(&env, 20, b);
    let malformed = seed(&env, 30, a);
    std::fs::write(malformed, "{").unwrap();
    let mismatch = seed(&env, 40, a);
    let mut value: Value = serde_json::from_slice(&std::fs::read(&mismatch).unwrap()).unwrap();
    value["timestamp"] = json!(41);
    std::fs::write(&mismatch, value.to_string()).unwrap();
    std::fs::write(
        mismatch
            .parent()
            .unwrap()
            .join(".activity-staging-test.json"),
        "{",
    )
    .unwrap();
    let output = env
        .command()
        .args([
            "activity",
            "list",
            "--context",
            "work",
            "--since",
            "20",
            "--limit",
            "2",
            "--json",
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("Warning"))
        .get_output()
        .stdout
        .clone();
    let entries: Vec<Value> = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![b, c]
    );
    assert!(entries.iter().all(|e| e["timestamp"] == 20));
    env.command()
        .args([
            "activity",
            "list",
            "--context",
            "work",
            "--limit",
            "0",
            "--json",
        ])
        .assert()
        .success()
        .stdout("[]\n");
}

#[test]
fn activity_round_trip_preserves_identity_and_provenance() {
    let env = fixture();
    env.command()
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "session_associated",
            "--details",
            "{}",
            "--session-id",
            "session-1",
            "--harness",
            "pi",
            "--host",
            "desktop",
        ])
        .assert()
        .success();
    let entries = events(&env);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry["kind"], "session_associated");
    assert_eq!(entry["session_id"], "session-1");
    assert_eq!(entry["harness"], "pi");
    assert_eq!(entry["host"], "desktop");
    assert_eq!(entry["details"], json!({}));
    assert!(entry["timestamp"].as_u64().unwrap() > 0);
    let id = entry["id"].as_str().unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(&id[14..15], "4");
}

#[test]
fn producers_choose_their_own_kinds() {
    let env = fixture();
    for kind in ["git_commit", "deploy_finished", "x1"] {
        env.command()
            .args([
                "activity",
                "add",
                "--context",
                "work",
                "--kind",
                kind,
                "--details",
                "{}",
            ])
            .assert()
            .success();
    }
    let kinds: Vec<_> = events(&env)
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds.len(), 3);
    for kind in ["git_commit", "deploy_finished", "x1"] {
        assert!(kinds.contains(&kind.to_string()));
    }
    env.command()
        .args(["activity", "list", "--context", "work"])
        .assert()
        .success()
        .stdout(predicates::str::contains("deploy_finished"));
}

#[test]
fn malformed_and_native_kinds_are_rejected_from_producers() {
    let env = fixture();
    for kind in [
        "",
        "Git_commit",
        "git commit",
        "1st",
        "git-commit",
        &"a".repeat(65),
    ] {
        env.command()
            .args([
                "activity",
                "add",
                "--context",
                "work",
                "--kind",
                kind,
                "--details",
                "{}",
            ])
            .assert()
            .failure();
    }
    env.command()
        .args([
            "activity",
            "add",
            "--context",
            "work",
            "--kind",
            "artifact_created",
            "--details",
            "{}",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("reserved"));
    assert!(events(&env).is_empty());
}
