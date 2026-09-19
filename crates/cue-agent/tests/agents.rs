//! Agent manifest discovery and layering, observed through the CLI.

mod helpers;

use helpers::Sandbox;

#[test]
fn project_prompt_sources_replace_the_other_global_form() {
    for (global, local) in [
        (
            r#"{"system_prompt_file":"missing.md","model":"test"}"#,
            r#"{"system_prompt":"local"}"#,
        ),
        (
            r#"{"system_prompt":"global","model":"test"}"#,
            r#"{"system_prompt_file":"local.md"}"#,
        ),
    ] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(&format!(r#"{{"agents":{{"a":{global}}}}}"#));
        sandbox.local_manifest(&format!(r#"{{"agents":{{"a":{local}}}}}"#));
        std::fs::write(sandbox.project().join("local.md"), "local").unwrap();
        let output = sandbox
            .cmd()
            .args(["agents", "list", "--json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["agents"][0]["system_prompt"], "local");
        assert_eq!(value["agents"][0]["model"], "test");
    }
}

#[test]
fn two_prompt_sources_in_one_layer_are_rejected_even_if_overridden() {
    let sandbox = Sandbox::new();
    sandbox
        .global_manifest(r#"{"agents":{"a":{"system_prompt":"x","system_prompt_file":"x.md"}}}"#);
    sandbox.local_manifest(r#"{"agents":{"a":{"system_prompt":"local"}}}"#);
    let output = sandbox.cmd().args(["agents", "list"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("sets both"), "{error}");
    assert!(error.contains("cue-agent.json"), "{error}");
}

#[test]
fn project_manifest_overrides_global_fields_without_restating_the_agent() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(
        r#"{
          "agents": {
            "explore": {
              "description": "Explores the codebase",
              "model": "anthropic/sonnet",
              "system_prompt": "You explore."
            },
            "consultant-opus": {
              "description": "Consults on hard problems",
              "model": "anthropic/opus",
              "system_prompt": "You consult."
            }
          }
        }"#,
    );
    sandbox.local_manifest(
        r#"{
          "agents": {
            "explore": { "model": "anthropic/haiku" }
          }
        }"#,
    );

    let output = sandbox
        .cmd()
        .args(["agents", "list", "--json"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    let agents = listed["agents"].as_array().expect("agents array");
    assert_eq!(agents.len(), 2);

    let explore = agents.iter().find(|a| a["name"] == "explore").unwrap();
    assert_eq!(explore["model"], "anthropic/haiku", "local layer overrides");
    assert_eq!(
        explore["description"], "Explores the codebase",
        "unstated fields survive the override"
    );
    assert_eq!(explore["system_prompt"], "You explore.");
    assert_eq!(explore["source"], "project");

    let consultant = agents
        .iter()
        .find(|a| a["name"] == "consultant-opus")
        .unwrap();
    assert_eq!(consultant["model"], "anthropic/opus");
    assert_eq!(consultant["source"], "user");
}

#[test]
fn a_manifest_keyed_by_name_is_required() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(r#"{ "agents": [{ "name": "explore" }] }"#);

    let output = sandbox
        .cmd()
        .args(["agents", "list"])
        .output()
        .expect("run cue-agent");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("object keyed by agent name"), "{stderr}");
}

#[test]
fn a_system_prompt_file_is_resolved_against_its_manifest() {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(sandbox.config().join("cue")).unwrap();
    std::fs::write(
        sandbox.config().join("cue").join("explore.md"),
        "From a file.\n",
    )
    .unwrap();
    sandbox.global_manifest(
        r#"{
          "agents": {
            "explore": {
              "description": "Explores",
              "system_prompt_file": "explore.md"
            }
          }
        }"#,
    );

    let output = sandbox
        .cmd()
        .args(["agents", "list", "--json"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(listed["agents"][0]["system_prompt"], "From a file.\n");
}

#[test]
fn listing_with_no_manifest_anywhere_is_empty_rather_than_an_error() {
    let sandbox = Sandbox::new();

    let output = sandbox
        .cmd()
        .args(["agents", "list", "--json"])
        .output()
        .expect("run cue-agent");

    assert!(output.status.success(), "{output:?}");
    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(listed["agents"].as_array().unwrap().len(), 0);
}
