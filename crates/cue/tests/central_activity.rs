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
            "git_commit_attempt",
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
            "git_commit_attempt",
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

fn seed(env: &TestEnv, timestamp: u64, id: &str) -> std::path::PathBuf {
    let dir = env.cue_store().join("acme/widgets/work/activity");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{timestamp:020}-{id}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "id": id, "timestamp": timestamp, "kind": "git_commit_attempt", "details": {}
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
