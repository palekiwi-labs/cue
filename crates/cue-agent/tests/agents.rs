//! Agent manifest discovery and layering, observed through the CLI.

mod helpers;

use helpers::Sandbox;
use serde_json::{Value, json};

/// `agents list --json`, asserting success.
fn listing(sandbox: &Sandbox) -> Value {
    let output = sandbox
        .cmd()
        .args(["agents", "list", "--json"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("json")
}

fn agent<'a>(listing: &'a Value, name: &str) -> &'a Value {
    listing["agents"]
        .as_array()
        .expect("agents array")
        .iter()
        .find(|agent| agent["name"] == name)
        .unwrap_or_else(|| panic!("no agent {name}: {listing}"))
}

/// `agents list`, asserting a usage failure, and returning stderr.
fn rejection(sandbox: &Sandbox) -> String {
    let output = sandbox
        .cmd()
        .args(["agents", "list"])
        .output()
        .expect("run cue-agent");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn inline_and_file_prompts_override_each_other_atomically() {
    for (global, local) in [
        (
            r#"{"system_prompt":{"file":"missing.md"},"model":"test"}"#,
            r#"{"system_prompt":"local"}"#,
        ),
        (
            r#"{"system_prompt":"global","model":"test"}"#,
            r#"{"system_prompt":{"file":"local.md"}}"#,
        ),
    ] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(&format!(r#"{{"agents":{{"a":{global}}}}}"#));
        sandbox.local_manifest(&format!(r#"{{"agents":{{"a":{local}}}}}"#));
        std::fs::write(sandbox.project().join("local.md"), "local").unwrap();
        let value = listing(&sandbox);
        assert_eq!(value["agents"][0]["system_prompt"], "local");
        assert_eq!(value["agents"][0]["model"], "test");
    }
}

#[test]
fn a_shadowed_invalid_field_is_still_rejected() {
    for global in [
        r#"{"model":5}"#,
        r#"{"system_prompt":{"file":"x.md","extra":true}}"#,
        r#"{"tools":["read",7]}"#,
    ] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(&format!(r#"{{"agents":{{"a":{global}}}}}"#));
        sandbox.local_manifest(
            r#"{"agents":{"a":{"model":"m","system_prompt":"local","tools":["read"]}}}"#,
        );
        let error = rejection(&sandbox);
        assert!(error.contains("cue-agent.json"), "{error}");
        assert!(error.contains("agents.a."), "{error}");
    }
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

    let listed = listing(&sandbox);
    assert_eq!(listed["agents"].as_array().unwrap().len(), 2);

    let explore = agent(&listed, "explore");
    assert_eq!(explore["model"], "anthropic/haiku", "local layer overrides");
    assert_eq!(
        explore["description"], "Explores the codebase",
        "unstated fields survive the override"
    );
    assert_eq!(explore["system_prompt"], "You explore.");
    assert_eq!(explore["source"], "project");
    assert_eq!(
        explore["field_sources"],
        json!({"description": "user", "model": "project", "system_prompt": "user"})
    );

    let consultant = agent(&listed, "consultant-opus");
    assert_eq!(consultant["model"], "anthropic/opus");
    assert_eq!(consultant["source"], "user");
}

#[test]
fn a_manifest_keyed_by_name_is_required() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(r#"{ "agents": [{ "name": "explore" }] }"#);
    let stderr = rejection(&sandbox);
    assert!(stderr.contains("object keyed by agent name"), "{stderr}");
}

#[test]
fn file_values_resolve_against_the_manifest_that_declared_them() {
    let sandbox = Sandbox::new();
    let global_dir = sandbox.config().join("cue");
    std::fs::create_dir_all(global_dir.join("prompts")).unwrap();
    std::fs::write(global_dir.join("prompts/explore.md"), "From a file.\n").unwrap();
    std::fs::write(sandbox.project().join("tool.txt"), "read").unwrap();
    sandbox.global_manifest(
        r#"{"agents":{"explore":{
          "description":"Explores",
          "system_prompt":{"file":"prompts/explore.md"}
        }}}"#,
    );
    sandbox.local_manifest(
        r#"{"agents":{"explore":{
          "model":{"file":"tool.txt"},
          "tools":[{"file":"tool.txt"},"bash"]
        }}}"#,
    );

    let listed = listing(&sandbox);
    let explore = agent(&listed, "explore");
    assert_eq!(explore["system_prompt"], "From a file.\n");
    assert_eq!(explore["model"], "read");
    assert_eq!(explore["tools"], json!(["read", "bash"]));
    assert_eq!(explore["field_sources"]["system_prompt"], "user");
    assert_eq!(explore["field_sources"]["tools"], "project");
}

#[test]
fn file_contents_are_literal_text_and_never_expanded() {
    let sandbox = Sandbox::new();
    let text = concat!(
        "# Role\n\n",
        "You say \"hello\" and 'bye'.\n",
        "Paths look like C:\\temp\\new and \\n is not a newline.\n",
        "```json\n{\"file\": \"other.md\"}\n```\n",
        "\n  trailing whitespace  \n\n",
    );
    std::fs::write(sandbox.project().join("prompt.md"), text).unwrap();
    std::fs::write(sandbox.project().join("other.md"), "EXPANDED").unwrap();
    sandbox.local_manifest(r#"{"agents":{"a":{"system_prompt":{"file":"prompt.md"}}}}"#);

    let listed = listing(&sandbox);
    assert_eq!(agent(&listed, "a")["system_prompt"], text);
}

#[test]
fn a_missing_referenced_file_is_an_error() {
    for definition in [
        r#"{"system_prompt":{"file":"missing.md"}}"#,
        r#"{"tools":["read",{"file":"missing.md"}]}"#,
    ] {
        let sandbox = Sandbox::new();
        sandbox.local_manifest(&format!(r#"{{"agents":{{"a":{definition}}}}}"#));
        let error = rejection(&sandbox);
        assert!(error.contains("missing.md"), "{error}");
    }
}

#[test]
fn agent_definitions_accept_only_reusable_fields() {
    for field in [
        r#""system_prompt_file":"x.md""#,
        r#""timeout_secs":5"#,
        r#""timeout":5"#,
        r#""env":{}"#,
        r#""prompt":"p""#,
        r#""cwd":"/tmp""#,
        r#""worktree":{}"#,
        r#""context":"a/b""#,
        r#""label":"l""#,
        r#""name":"a""#,
        r#""bogus":1"#,
    ] {
        let sandbox = Sandbox::new();
        sandbox.local_manifest(&format!(r#"{{"agents":{{"a":{{{field}}}}}}}"#));
        let error = rejection(&sandbox);
        assert!(error.contains("unknown field"), "{field}: {error}");
    }
}

#[test]
fn agent_field_types_are_validated() {
    for definition in [
        r#"{"description":1}"#,
        r#"{"model":true}"#,
        r#"{"thinking":["high"]}"#,
        r#"{"system_prompt":{"path":"x.md"}}"#,
        r#"{"tools":"read"}"#,
        r#"{"tools":{"file":"tools.txt"}}"#,
        r#"{"tools":[null]}"#,
        r#"{"tools":[["read"]]}"#,
    ] {
        let sandbox = Sandbox::new();
        sandbox.local_manifest(&format!(r#"{{"agents":{{"a":{definition}}}}}"#));
        let error = rejection(&sandbox);
        assert!(error.contains("agents.a."), "{definition}: {error}");
    }
}

#[test]
fn the_root_accepts_only_supervisor_fields() {
    for root in [
        r#"{"settings":{}}"#,
        r#"{"harness":"pi"}"#,
        r#"{"defaults":{}}"#,
        r#"{"timeout":"5"}"#,
        r#"{"timeout":-1}"#,
        r#"{"timeout":1.5}"#,
        r#"{"timeout":null}"#,
        r#"{"timeout":{"file":"timeout.txt"}}"#,
        r#"{"worktree_root":3}"#,
        r#"{"worktree_root":{"file":"missing.txt"}}"#,
        r#"{"agents":null}"#,
        r#"{"agents":{"a":null}}"#,
        r#"{"agents":{"a":"text"}}"#,
        r#"[]"#,
    ] {
        let sandbox = Sandbox::new();
        std::fs::write(sandbox.project().join("timeout.txt"), "5").unwrap();
        sandbox.local_manifest(root);
        let error = rejection(&sandbox);
        assert!(error.contains("cue-agent.json"), "{root}: {error}");
    }
}

#[test]
fn valid_root_fields_load() {
    let sandbox = Sandbox::new();
    std::fs::write(sandbox.project().join("root.txt"), "../trees").unwrap();
    sandbox.global_manifest(r#"{"timeout":30,"worktree_root":"/tmp/trees"}"#);
    sandbox
        .local_manifest(r#"{"timeout":0,"worktree_root":{"file":"root.txt"},"agents":{"a":{}}}"#);
    assert_eq!(listing(&sandbox)["agents"].as_array().unwrap().len(), 1);

    sandbox.local_manifest(r#"{"worktree_root":null}"#);
    assert_eq!(listing(&sandbox)["agents"].as_array().unwrap().len(), 0);
}

#[test]
fn tools_distinguish_unspecified_empty_and_selected() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(
        r#"{"agents":{
          "unset":{},
          "none":{"tools":[]},
          "some":{"tools":["read","bash"]},
          "cleared":{"tools":["read"]},
          "replaced":{"tools":["read","bash"]}
        }}"#,
    );
    sandbox.local_manifest(
        r#"{"agents":{
          "cleared":{"tools":null},
          "replaced":{"tools":["grep"]}
        }}"#,
    );

    let listed = listing(&sandbox);
    assert_eq!(agent(&listed, "unset")["tools"], Value::Null);
    assert_eq!(agent(&listed, "none")["tools"], json!([]));
    assert_eq!(agent(&listed, "some")["tools"], json!(["read", "bash"]));
    assert_eq!(agent(&listed, "cleared")["tools"], Value::Null);
    assert_eq!(agent(&listed, "replaced")["tools"], json!(["grep"]));
}

#[test]
fn null_clears_inherited_optional_values() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(
        r#"{"agents":{"a":{
          "description":"d","model":"m","system_prompt":{"file":"missing.md"},
          "thinking":"high","tools":["read"]
        }}}"#,
    );
    sandbox.local_manifest(
        r#"{"agents":{"a":{
          "description":null,"model":null,"system_prompt":null,
          "thinking":null,"tools":null
        }}}"#,
    );

    let listed = listing(&sandbox);
    let a = agent(&listed, "a");
    assert_eq!(a["description"], Value::Null);
    assert_eq!(a["model"], Value::Null);
    assert_eq!(a["system_prompt"], "");
    assert_eq!(a["thinking"], Value::Null);
    assert_eq!(a["tools"], Value::Null);
    assert_eq!(a["field_sources"], json!({}));
    assert!(a.get("timeout_secs").is_none(), "{a}");
}

#[test]
fn listing_with_no_manifest_anywhere_is_empty_rather_than_an_error() {
    let sandbox = Sandbox::new();
    let listed = listing(&sandbox);
    assert_eq!(listed["agents"].as_array().unwrap().len(), 0);
}
