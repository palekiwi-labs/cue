//! Running named agents, observed at the process boundary.

mod helpers;

use helpers::{Sandbox, receipt, run_of, spec};
use serde_json::json;

const MANIFEST: &str = r#"{
  "agents": {
    "explore": {
      "description": "Explores the codebase",
      "model": "anthropic/haiku",
      "system_prompt": "You explore.",
      "tools": ["read", "bash"]
    },
    "consultant-opus": {
      "description": "Consults",
      "model": "anthropic/opus",
      "system_prompt": "You consult."
    },
    "bare": {
      "description": "Inherits every harness default"
    }
  }
}"#;

/// Install a `pi` on the sandbox `PATH` that answers `--version` with `body`
/// and otherwise delegates to the fake harness.
fn install_version_wrapper(sandbox: &Sandbox, body: &str) {
    sandbox.install_pi(&format!(
        "#!/usr/bin/env bash\nif [[ $1 == --version ]]; then {body}; exit 0; fi\nexec '{}' \"$@\"\n",
        sandbox.harness().display()
    ));
}

#[test]
fn version_probe_does_not_wait_for_inherited_stdout_or_buffer_large_output() {
    for body in ["sleep 4 &\nprintf 'wrapper 1\\n'", "printf '%01000000d' 0"] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(MANIFEST);
        install_version_wrapper(&sandbox, body);
        let started = std::time::Instant::now();
        let output = sandbox.run_json(&spec(&[("bare", "hello")]));
        assert!(output.status.success(), "{output:?}");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        let version = receipt(&output.stdout)["harness_version"].clone();
        if body.starts_with("sleep") {
            assert_eq!(version, "wrapper 1");
        } else {
            assert!(version.is_null());
        }
    }
}

#[test]
fn private_run_records_and_zero_timeout_are_recorded_consistently() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let root = sandbox.state().join("cue/agent");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = sandbox.cmd();
    // SAFETY: umask is async-signal-safe; this only changes the child process.
    unsafe {
        command.pre_exec(|| {
            libc::umask(0o022);
            Ok(())
        });
    }
    let output = command
        .args(["run", "--json", "--timeout", "0"])
        .arg(spec(&[("bare", "hello")]).to_string())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let path = std::path::Path::new(receipt["runs"][0]["run_path"].as_str().unwrap());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest["timeout_secs"].is_null());
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in [
        "prompt.md",
        "system-prompt.md",
        "manifest.json",
        "receipt.json",
        "events.jsonl",
        "stderr.log",
    ] {
        assert_eq!(
            std::fs::metadata(path.join(file))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "{file}"
        );
    }
    assert_eq!(
        std::fs::metadata(root.join("index.jsonl"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn a_stuck_version_probe_is_best_effort_and_does_not_block_the_run() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    install_version_wrapper(&sandbox, "sleep 4");
    let started = std::time::Instant::now();
    let output = sandbox.run_json(&spec(&[("bare", "hello")]));
    assert!(output.status.success(), "{output:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert!(receipt(&output.stdout)["harness_version"].is_null());
}

#[test]
fn the_harness_runs_in_the_task_cwd_found_through_path() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let target = sandbox.project().join("nested");
    std::fs::create_dir(&target).unwrap();
    let output = sandbox.run_json(&json!({
        "tasks": [{"agent": "bare", "prompt": "hello", "cwd": target}]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    assert_eq!(receipt["harness"], "pi");
    assert_eq!(receipt["harness_version"], "fake-pi 9.9.9");
    let run = &receipt["runs"][0];
    let id = run["run_id"].as_str().unwrap();
    assert_eq!(sandbox.recorded_cwd(id), target.to_str().unwrap());
    let path = std::path::Path::new(run["run_path"].as_str().unwrap());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["cwd"], target.display().to_string());
    assert_eq!(
        manifest["harness_path"],
        sandbox.pi_dir().join("pi").display().to_string()
    );
}

#[test]
fn the_removed_harness_override_variable_is_ignored() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let output = sandbox
        .cmd()
        .env("CUE_AGENT_HARNESS", sandbox.project().join("no-such-pi"))
        .args(["run", "--json"])
        .arg(spec(&[("bare", "hello")]).to_string())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        run_of(&receipt(&output.stdout), "bare")["response"],
        "echo: hello"
    );
}

#[test]
fn persistence_failures_return_results_instead_of_admission_errors() {
    for block_receipt in [false, true] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(MANIFEST);
        if !block_receipt {
            std::fs::create_dir_all(sandbox.state().join("cue/agent/index.jsonl")).unwrap();
        }
        let prompt = if block_receipt {
            "BLOCK_RECEIPT"
        } else {
            "hello"
        };
        let output = sandbox.run_json(&spec(&[("explore", prompt), ("consultant-opus", prompt)]));
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let receipt = receipt(&output.stdout);
        assert_eq!(receipt["runs"].as_array().unwrap().len(), 2);
        for run in receipt["runs"].as_array().unwrap() {
            assert_eq!(run["outcome"], "completed");
            assert!(!run["response"].as_str().unwrap().is_empty());
            let errors = run["persistence_errors"].as_array().unwrap();
            assert_eq!(errors.len(), 1);
            assert!(errors[0].as_str().unwrap().contains(if block_receipt {
                "receipt.json"
            } else {
                "index.jsonl"
            }));
        }
    }
}

#[test]
fn unsafe_prompts_are_rejected_before_any_run_in_the_batch() {
    for prompt in ["--version", "-x", "@secret", "   "] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(MANIFEST);
        let output = sandbox.run_json(&spec(&[("bare", "safe"), ("bare", prompt)]));
        sandbox.assert_nothing_ran(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("tasks[1].prompt"), "{stderr}");
    }
}

#[test]
fn a_named_agent_runs_with_its_model_prompt_and_system_prompt() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox.run_json(&spec(&[("explore", "Find the bug")]));
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    assert_eq!(run["outcome"], "completed");
    assert_eq!(run["exit_code"], 0);
    assert_eq!(run["response"], "echo: Find the bug");
    assert_eq!(run["model"], "anthropic/haiku");
    assert_eq!(run["turns"], 1);
    assert_eq!(run["tokens_input"], 11);
    assert_eq!(run["tokens_output"], 22);
    assert_eq!(run["cost_usd"], 0.03);
    assert_eq!(receipt["cap"], 4);
    assert_eq!(receipt["harness_version"], "fake-pi 9.9.9");
    assert!(
        receipt.get("context").is_none(),
        "a batch has no single context: {receipt}"
    );

    let run_id = run["run_id"].as_str().expect("run id");
    let run_path = std::path::PathBuf::from(run["run_path"].as_str().expect("run path"));

    assert_eq!(
        sandbox.recorded_argv(run_id),
        vec![
            "--mode".to_string(),
            "json".to_string(),
            "-p".to_string(),
            "--no-session".to_string(),
            "--session-id".to_string(),
            run_id.to_string(),
            "--model".to_string(),
            "anthropic/haiku".to_string(),
            "--tools".to_string(),
            "read,bash".to_string(),
            "--append-system-prompt".to_string(),
            run_path.join("system-prompt.md").display().to_string(),
            "Find the bug".to_string(),
        ],
        "argv must name the intended model, tools, system prompt and prompt"
    );

    assert_eq!(
        std::fs::read_to_string(run_path.join("prompt.md")).unwrap(),
        "Find the bug",
        "the prompt is recorded verbatim as it was passed"
    );
    assert_eq!(
        std::fs::read_to_string(run_path.join("system-prompt.md")).unwrap(),
        "You explore.",
        "the system prompt is recorded as it was at run time"
    );

    let events = std::fs::read_to_string(run_path.join("events.jsonl")).unwrap();
    assert!(events.contains("message_end"), "{events}");
    assert!(run_path.join("stderr.log").is_file());

    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_path.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(written["run_id"], run_id);
    assert_eq!(written["outcome"], "completed");

    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_path.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(request["agent"], "explore");
    assert_eq!(request["model"], "anthropic/haiku");
    assert_eq!(request["context"], serde_json::Value::Null);
    assert_eq!(request["argv"][0], "--mode");
    assert_eq!(request["store_root"], sandbox.store().display().to_string());

    let index =
        std::fs::read_to_string(sandbox.state().join("cue/agent/index.jsonl")).expect("index");
    assert_eq!(index.lines().count(), 1);
    let entry: serde_json::Value = serde_json::from_str(index.lines().next().unwrap()).unwrap();
    assert_eq!(entry["run_id"], run_id);
    assert_eq!(entry["outcome"], "completed");
    assert_eq!(entry["path"], run_path.display().to_string());
}

#[test]
fn a_prompt_is_passed_as_one_verbatim_argument() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let prompt = "Line one\n  \"quoted\" $HOME `tick` \\ back\n--not-a-flag\n";
    let output = sandbox.run_json(&spec(&[("bare", prompt)]));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "bare");
    let run_path = std::path::PathBuf::from(run["run_path"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(run_path.join("prompt.md")).unwrap(),
        prompt
    );
    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_path.join("manifest.json")).unwrap()).unwrap();
    let argv = request["argv"].as_array().unwrap();
    assert_eq!(argv.last().unwrap(), prompt);
}

#[test]
fn an_agent_with_no_model_or_system_prompt_inherits_the_harness_defaults() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox.run_json(&spec(&[("bare", "hello")]));
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run_id = run_of(&receipt, "bare")["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        sandbox.recorded_argv(&run_id),
        vec![
            "--mode",
            "json",
            "-p",
            "--no-session",
            "--session-id",
            &run_id,
            "hello",
        ],
        "no model, no tools and an empty system prompt add no arguments"
    );
}

/// The tool arguments pi received for one run of `agent`.
fn tool_args(sandbox: &Sandbox, agent: &str) -> Vec<String> {
    let output = sandbox.run_json(&spec(&[(agent, "hello")]));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run_id = run_of(&receipt, agent)["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let argv = sandbox.recorded_argv(&run_id);
    let mut tools = Vec::new();
    let mut iter = argv.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--no-tools" => tools.push(arg.clone()),
            "--tools" => {
                tools.push(arg.clone());
                tools.push(iter.next().unwrap().clone());
            }
            _ => {}
        }
    }
    tools
}

#[test]
fn tools_select_pi_defaults_none_or_an_explicit_list() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(
        r#"{"agents":{
          "unset":{},
          "none":{"tools":[]},
          "some":{"tools":["read",{"file":"tool.txt"}]},
          "cleared":{"tools":["read"]}
        }}"#,
    );
    std::fs::write(sandbox.config().join("cue/tool.txt"), "grep").unwrap();
    sandbox.local_manifest(r#"{"agents":{"cleared":{"tools":null}}}"#);

    assert!(tool_args(&sandbox, "unset").is_empty());
    assert_eq!(tool_args(&sandbox, "none"), vec!["--no-tools"]);
    assert_eq!(tool_args(&sandbox, "some"), vec!["--tools", "read,grep"]);
    assert!(tool_args(&sandbox, "cleared").is_empty());
}

#[test]
fn a_file_sourced_system_prompt_is_appended_verbatim() {
    assert_appended_prompt("# Role\n\nSay \"hi\" \\ and {\"file\": \"x.md\"}\n\n");
}

#[test]
fn whitespace_only_file_instructions_are_appended_verbatim() {
    assert_appended_prompt(" \t\n\n");
}

fn assert_appended_prompt(text: &str) {
    let sandbox = Sandbox::new();
    std::fs::write(sandbox.project().join("role.md"), text).unwrap();
    sandbox.local_manifest(r#"{"agents":{"a":{"system_prompt":{"file":"role.md"}}}}"#);

    let output = sandbox.run_json(&spec(&[("a", "hello")]));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "a");
    let run_path = std::path::PathBuf::from(run["run_path"].as_str().unwrap());
    let argv = sandbox.recorded_argv(run["run_id"].as_str().unwrap());
    let position = argv
        .iter()
        .position(|arg| arg == "--append-system-prompt")
        .expect("appended system prompt");
    assert_eq!(
        argv[position + 1],
        run_path.join("system-prompt.md").display().to_string()
    );
    assert!(!argv.iter().any(|arg| arg == "--system-prompt"), "{argv:?}");
    assert_eq!(
        std::fs::read_to_string(run_path.join("system-prompt.md")).unwrap(),
        text
    );
}

#[test]
fn an_unknown_agent_is_refused_before_anything_is_spawned() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox.run_json(&spec(&[("explore", "hi"), ("nope", "hi")]));
    sandbox.assert_nothing_ran(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown agent 'nope'"), "{stderr}");
    assert!(
        stderr.contains("explore"),
        "the alternatives are named: {stderr}"
    );
}

#[test]
fn the_harness_sees_an_immediately_closed_stdin() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let started = std::time::Instant::now();
    let output = sandbox.run_json(&spec(&[("explore", "STDIN")]));
    let elapsed = started.elapsed();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        run_of(&receipt(&output.stdout), "explore")["response"],
        "stdin bytes:0",
        "the harness must never wait on input"
    );
    assert!(elapsed < std::time::Duration::from_secs(5), "{elapsed:?}");
}
