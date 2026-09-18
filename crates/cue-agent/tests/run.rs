//! Running named agents, observed at the process boundary.

mod helpers;

use helpers::{Sandbox, receipt, run_of};

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

#[test]
fn version_probe_does_not_wait_for_inherited_stdout_or_buffer_large_output() {
    for body in ["sleep 4 &\nprintf 'wrapper 1\\n'", "printf '%01000000d' 0"] {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(MANIFEST);
        let real = sandbox.harness();
        let wrapper = sandbox.project().join("wrapper");
        helpers::write_executable(
            &wrapper,
            &format!(
                "#!/usr/bin/env bash\nif [[ $1 == --version ]]; then {body}; exit 0; fi\nexec '{}' \"$@\"\n",
                real.display()
            ),
        );
        let started = std::time::Instant::now();
        let output = sandbox
            .cmd()
            .args(["run", "bare", "--prompt", "hello", "--harness"])
            .arg(wrapper)
            .output()
            .unwrap();
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
        .args(["run", "bare", "--prompt", "hello", "--timeout", "0"])
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
    let real = sandbox.harness();
    let wrapper = sandbox.project().join("wrapper");
    helpers::write_executable(
        &wrapper,
        &format!(
            "#!/usr/bin/env bash\nif [[ $1 == --version ]]; then sleep 4; exit 0; fi\nexec '{}' \"$@\"\n",
            real.display()
        ),
    );
    let started = std::time::Instant::now();
    let output = sandbox
        .cmd()
        .args(["run", "bare", "--prompt", "hello", "--harness"])
        .arg(wrapper)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert!(receipt(&output.stdout)["harness_version"].is_null());
}

#[test]
fn a_relative_harness_uses_one_program_and_the_selected_cwd() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let target = sandbox.project().join("nested");
    std::fs::create_dir(&target).unwrap();
    let output = sandbox
        .cmd()
        .args([
            "run",
            "bare",
            "--prompt",
            "hello",
            "--harness",
            "../fake-pi",
            "--cwd",
        ])
        .arg(&target)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    assert_eq!(receipt["harness_version"], "fake-pi 9.9.9");
    let id = receipt["runs"][0]["run_id"].as_str().unwrap();
    assert_eq!(
        std::fs::read_to_string(sandbox.harness_log().join(format!("{id}.cwd")))
            .unwrap()
            .trim(),
        target.to_str().unwrap()
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
        let output = sandbox
            .cmd()
            .args([
                "run",
                "explore",
                "consultant-opus",
                "--prompt",
                if block_receipt {
                    "BLOCK_RECEIPT"
                } else {
                    "hello"
                },
            ])
            .output()
            .unwrap();
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
        let batch = sandbox.project().join("batch.json");
        std::fs::write(
            &batch,
            serde_json::to_vec(&serde_json::json!([
                {"agent":"bare", "prompt":"safe"},
                {"agent":"bare", "prompt":prompt}
            ]))
            .unwrap(),
        )
        .unwrap();
        let output = sandbox
            .cmd()
            .args(["run", "--batch"])
            .arg(batch)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(sandbox.recorded_sessions().is_empty());
        assert!(!sandbox.state().join("cue/agent/runs").exists());
    }
}

#[test]
fn a_named_agent_runs_with_its_model_prompt_and_system_prompt() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .args(["run", "explore", "--prompt", "Find the bug"])
        .output()
        .expect("run cue-agent");
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
fn an_agent_with_no_model_or_system_prompt_inherits_the_harness_defaults() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .args(["run", "bare", "--prompt", "hello"])
        .output()
        .expect("run cue-agent");
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

#[test]
fn an_unknown_agent_is_refused_before_anything_is_spawned() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .args(["run", "explore", "nope", "--prompt", "hi"])
        .output()
        .expect("run cue-agent");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown agent 'nope'"), "{stderr}");
    assert!(
        stderr.contains("explore"),
        "the alternatives are named: {stderr}"
    );
    assert!(
        sandbox.recorded_sessions().is_empty(),
        "no harness process may start when the batch is refused"
    );
}

#[test]
fn the_harness_sees_an_immediately_closed_stdin() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let started = std::time::Instant::now();
    let output = sandbox
        .cmd()
        .args(["run", "explore", "--prompt", "STDIN"])
        .output()
        .expect("run cue-agent");
    let elapsed = started.elapsed();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        run_of(&receipt(&output.stdout), "explore")["response"],
        "stdin bytes:0",
        "the harness must never wait on input"
    );
    assert!(elapsed < std::time::Duration::from_secs(5), "{elapsed:?}");
}

#[test]
fn the_prompt_can_come_from_a_file_or_from_standard_input() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let prompt_path = sandbox.project().join("prompt.txt");
    std::fs::write(&prompt_path, "from a file").unwrap();

    let output = sandbox
        .cmd()
        .args(["run", "explore", "--prompt-file"])
        .arg(&prompt_path)
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        run_of(&receipt(&output.stdout), "explore")["response"],
        "echo: from a file"
    );
}
