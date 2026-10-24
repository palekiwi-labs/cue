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
