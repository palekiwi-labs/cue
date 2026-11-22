mod helpers;

use helpers::TestEnv;
use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const RECORDER: &str = r#"#!/bin/sh
n=$(ls "$HOOK_OUT" | wc -l)
f="$HOOK_OUT/$(printf %03d "$n")-$(basename "$0")"
cat > "$f.json"
{
  echo "event=$CUE_HOOK_EVENT"
  echo "event_id=$CUE_HOOK_EVENT_ID"
  echo "parent=${CUE_HOOK_PARENT_EVENT_ID-<unset>}"
  echo "root=$CUE_HOOK_ROOT_EVENT_ID"
  echo "depth=$CUE_HOOK_DEPTH"
  echo "store=$CUE_STORE"
} > "$f.env"
"#;

struct Fixture {
    env: TestEnv,
    out: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let env = TestEnv::new();
        env.setup_repo_with_origin();
        let out = env.temp_dir.path().join("hook-out");
        fs::create_dir_all(&out).unwrap();
        let fixture = Self { env, out };
        fixture
            .cue()
            .args(["context", "create", "work"])
            .assert()
            .success();
        fixture
    }

    fn cue(&self) -> assert_cmd::Command {
        let mut cmd = self.env.command();
        cmd.env("HOOK_OUT", &self.out);
        cmd
    }

    fn hook(&self, dir: &str, name: &str, body: &str) -> PathBuf {
        let dir = self.env.config_dir.join("hooks").join(dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn calls(&self) -> Vec<(String, Value, String)> {
        let mut names: Vec<_> = fs::read_dir(&self.out)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.ends_with(".json"))
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let stem = name.trim_end_matches(".json").to_string();
                let payload =
                    serde_json::from_str(&fs::read_to_string(self.out.join(&name)).unwrap())
                        .unwrap();
                let env = fs::read_to_string(self.out.join(format!("{stem}.env"))).unwrap();
                let hook = stem.split_once('-').unwrap().1.to_string();
                (hook, payload, env)
            })
            .collect()
    }

    fn dispatch_log(&self) -> Vec<Value> {
        let path = self.env.temp_dir.path().join("state/cue/hooks.jsonl");
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn add_note(&self, name: &str) -> assert_cmd::assert::Assert {
        self.cue()
            .args(["add", name, "body", "--type", "note", "--context", "work"])
            .assert()
    }
}

fn env_value<'a>(env: &'a str, key: &str) -> &'a str {
    env.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap()
}

#[test]
fn all_three_events_reach_a_hook_with_minimal_payload() {
    let fx = Fixture::new();
    fx.hook("all.d", "rec", RECORDER);
    let scope = "acme/widgets";

    fx.cue()
        .args(["context", "create", "other"])
        .assert()
        .success();
    fx.add_note("idea").success();
    fx.cue()
        .args(["log", "add", "--title", "hello", "--context", "work"])
        .assert()
        .success();

    let calls = fx.calls();
    assert_eq!(calls.len(), 3);
    let events: Vec<_> = calls
        .iter()
        .map(|(_, payload, _)| payload["event"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        events,
        ["context_create", "artifact_create", "log_entry_add"]
    );

    assert_eq!(calls[0].1["address"], format!("{scope}/other"));
    assert_eq!(calls[1].1["address"], format!("{scope}/work/note/idea.md"));
    let log_address = calls[2].1["address"].as_str().unwrap();
    assert!(log_address.starts_with(&format!("{scope}/work/log/")));
    assert!(log_address.ends_with(".json"));

    for (_, payload, env) in &calls {
        let object = payload.as_object().unwrap();
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["address", "event", "event_id", "occurred_at", "v"]);
        assert_eq!(payload["v"], 1);
        assert_eq!(env_value(env, "event"), payload["event"]);
        assert_eq!(env_value(env, "event_id"), payload["event_id"]);
        assert_eq!(env_value(env, "root"), payload["event_id"]);
        assert_eq!(env_value(env, "parent"), "<unset>");
        assert_eq!(env_value(env, "depth"), "1");
        assert_eq!(
            env_value(env, "store"),
            fx.env.cue_store().to_str().unwrap()
        );
        let occurred_at = payload["occurred_at"].as_str().unwrap();
        assert!(occurred_at.ends_with('Z') && occurred_at.contains('T'));
    }
}

#[test]
fn discovery_merges_all_and_event_dirs_in_lexical_order() {
    let fx = Fixture::new();
    fx.hook("all.d", "20-b", RECORDER);
    fx.hook("artifact_create.d", "10-a", RECORDER);
    fx.hook("artifact_create.d", "20-b", RECORDER);
    fx.hook("all.d", "30-c", RECORDER);
    fx.hook("log_entry_add.d", "15-other", RECORDER);
    fx.hook("all.d", ".hidden", RECORDER);
    fx.hook("all.d", "40-d~", RECORDER);
    fx.hook("all.d", "50-e.bak", RECORDER);
    let not_exec = fx.hook("all.d", "60-f", RECORDER);
    fs::set_permissions(&not_exec, fs::Permissions::from_mode(0o644)).unwrap();
    fs::create_dir_all(fx.env.config_dir.join("hooks/all.d/70-dir")).unwrap();

    fx.add_note("idea").success();

    let order: Vec<_> = fx.calls().into_iter().map(|(hook, _, _)| hook).collect();
    assert_eq!(order, ["10-a", "20-b", "20-b", "30-c"]);
    let log = fx.dispatch_log();
    let paths: Vec<_> = log
        .iter()
        .map(|record| record["hook"].as_str().unwrap().to_string())
        .collect();
    assert!(paths[1].contains("all.d/20-b"));
    assert!(paths[2].contains("artifact_create.d/20-b"));
}

#[test]
fn symlinked_hooks_run() {
    let fx = Fixture::new();
    let target = fx.env.temp_dir.path().join("real-hook");
    fs::write(&target, RECORDER).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    let dir = fx.env.config_dir.join("hooks/all.d");
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&target, dir.join("link")).unwrap();

    fx.add_note("idea").success();
    assert_eq!(fx.calls().len(), 1);
}

#[test]
fn failing_hook_is_non_fatal_and_output_is_isolated() {
    let fx = Fixture::new();
    fx.hook(
        "all.d",
        "10-fail",
        "#!/bin/sh\necho hook-stdout\necho hook-stderr >&2\nexit 7\n",
    );
    fx.hook("all.d", "20-rec", RECORDER);

    let output = fx.add_note("idea").success().get_output().clone();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stdout.contains("hook-stdout"));
    assert!(!stderr.contains("hook-stderr"));
    assert!(stderr.contains("10-fail exited with 7"), "{stderr}");
    assert!(
        fx.env
            .cue_store()
            .join("acme/widgets/work/note/idea.md")
            .is_file()
    );
    assert_eq!(fx.calls().len(), 1);

    let log = fx.dispatch_log();
    assert_eq!(log[0]["outcome"], "failed");
    assert_eq!(log[0]["exit_code"], 7);
    assert_eq!(log[0]["stderr_tail"], "hook-stderr");
    assert_eq!(log[1]["outcome"], "ok");
}

#[test]
fn slow_hook_is_killed_with_its_process_group() {
    let fx = Fixture::new();
    let marker = fx.env.temp_dir.path().join("grandchild-survived");
    fx.hook(
        "all.d",
        "10-slow",
        &format!(
            "#!/bin/sh\n(sleep 1; touch {}) &\nsleep 30\n",
            marker.display()
        ),
    );
    fx.hook("all.d", "20-rec", RECORDER);

    let started = Instant::now();
    let output = fx
        .cue()
        .env("CUE_HOOK_TIMEOUT_MS", "200")
        .args(["add", "idea", "body", "--type", "note", "--context", "work"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(started.elapsed() < Duration::from_secs(5));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("10-slow timed out"), "{stderr}");
    assert_eq!(fx.calls().len(), 1);
    assert_eq!(fx.dispatch_log()[0]["outcome"], "timeout");

    std::thread::sleep(Duration::from_millis(1500));
    assert!(!marker.exists(), "process group was not killed");
}

#[test]
fn detached_background_work_does_not_block() {
    let fx = Fixture::new();
    fx.hook("all.d", "10-detach", "#!/bin/sh\nsleep 3 &\nexit 0\n");

    let started = Instant::now();
    fx.cue()
        .env("CUE_HOOK_TIMEOUT_MS", "2000")
        .args(["add", "idea", "body", "--type", "note", "--context", "work"])
        .assert()
        .success();
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert_eq!(fx.dispatch_log()[0]["outcome"], "ok");
}

#[test]
fn budget_skips_remaining_hooks() {
    let fx = Fixture::new();
    fx.hook("all.d", "10-slow", "#!/bin/sh\nsleep 30\n");
    fx.hook("all.d", "20-rec", RECORDER);

    fx.cue()
        .env("CUE_HOOK_TIMEOUT_MS", "5000")
        .env("CUE_HOOK_BUDGET_MS", "200")
        .args(["add", "idea", "body", "--type", "note", "--context", "work"])
        .assert()
        .success();
    assert!(fx.calls().is_empty());
    let outcomes: Vec<_> = fx
        .dispatch_log()
        .iter()
        .map(|record| record["outcome"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(outcomes, ["timeout", "budget_exhausted"]);
}

#[test]
fn nested_writes_carry_lineage_and_stop_at_the_depth_cap() {
    let fx = Fixture::new();
    let cue = assert_cmd::cargo::cargo_bin("cue");
    fx.hook("all.d", "10-rec", RECORDER);
    fx.hook(
        "artifact_create.d",
        "20-cascade",
        &format!(
            "#!/bin/sh\n\"{}\" add \"n$CUE_HOOK_DEPTH\" body --type note --context work >/dev/null 2>&1\n",
            cue.display()
        ),
    );

    fx.cue()
        .env("CUE_HOOK_MAX_DEPTH", "3")
        .args(["add", "seed", "body", "--type", "note", "--context", "work"])
        .assert()
        .success();

    let calls = fx.calls();
    let depths: Vec<_> = calls
        .iter()
        .map(|(_, _, env)| env_value(env, "depth").to_string())
        .collect();
    assert_eq!(depths, ["1", "2", "3"]);
    let root = calls[0].1["event_id"].as_str().unwrap();
    assert_eq!(env_value(&calls[0].2, "parent"), "<unset>");
    for pair in calls.windows(2) {
        assert_eq!(env_value(&pair[1].2, "parent"), pair[0].1["event_id"]);
    }
    for (_, _, env) in &calls {
        assert_eq!(env_value(env, "root"), root);
    }

    let store = fx.env.cue_store().join("acme/widgets/work/note");
    assert!(store.join("n3.md").is_file());
    assert!(!store.join("n4.md").exists());
    let capped: Vec<_> = fx
        .dispatch_log()
        .into_iter()
        .filter(|record| record["outcome"] == "depth_exceeded")
        .collect();
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0]["depth"], 4);
    assert_eq!(capped[0]["root_event_id"], root);
}

#[test]
fn forced_overwrite_fires_nothing() {
    let fx = Fixture::new();
    fx.add_note("idea").success();
    fx.hook("all.d", "rec", RECORDER);

    fx.cue()
        .args([
            "add",
            "idea",
            "new",
            "--type",
            "note",
            "--context",
            "work",
            "--force",
        ])
        .assert()
        .success();
    fx.cue()
        .args([
            "add",
            "fresh",
            "new",
            "--type",
            "note",
            "--context",
            "work",
            "--force",
        ])
        .assert()
        .success();

    let calls = fx.calls();
    assert_eq!(calls.len(), 1);
    assert!(
        calls[0].1["address"]
            .as_str()
            .unwrap()
            .ends_with("note/fresh.md")
    );
}

#[test]
fn failed_write_fires_nothing() {
    let fx = Fixture::new();
    fx.hook("all.d", "rec", RECORDER);
    fx.cue()
        .args([
            "add",
            "idea",
            "body",
            "--type",
            "note",
            "--context",
            "missing",
        ])
        .assert()
        .failure();
    fx.cue()
        .args(["context", "create", "work"])
        .assert()
        .failure();
    assert!(fx.calls().is_empty());
}

#[test]
fn no_hooks_dir_is_a_no_op() {
    let fx = Fixture::new();
    fx.add_note("idea").success();
    assert!(fx.dispatch_log().is_empty());
}

#[test]
fn every_artifact_type_fires_artifact_create() {
    let fx = Fixture::new();
    fx.hook("artifact_create.d", "rec", RECORDER);

    fx.cue()
        .args(["add", "probe", "-", "--type", "bin", "--context", "work"])
        .write_stdin("#!/bin/sh\n")
        .assert()
        .success();
    fx.cue()
        .args([
            "add",
            "findings",
            "{}",
            "--type",
            "review",
            "--context",
            "work",
        ])
        .assert()
        .success();
    fx.cue()
        .args(["add", "dump.txt", "x", "--type", "tmp", "--context", "work"])
        .assert()
        .success();

    let addresses: Vec<_> = fx
        .calls()
        .into_iter()
        .map(|(_, payload, _)| payload["address"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(addresses.len(), 3, "{addresses:?}");
    assert_eq!(addresses[0], "acme/widgets/work/bin/probe");
    assert_eq!(addresses[1], "acme/widgets/work/review/findings.json");
    assert!(addresses[2].starts_with("acme/widgets/work/tmp/"));
    assert!(addresses[2].ends_with("/dump.txt"));
}

#[test]
fn stderr_is_kept_when_a_background_job_holds_the_pipe() {
    let fx = Fixture::new();
    fx.hook(
        "all.d",
        "10-fail",
        "#!/bin/sh\necho before-exit >&2\nsleep 3 &\nexit 2\n",
    );

    fx.add_note("idea").success();
    let log = fx.dispatch_log();
    assert_eq!(log[0]["outcome"], "failed");
    assert_eq!(log[0]["stderr_tail"], "before-exit");
}

#[test]
fn timeout_reaches_hooks_of_nested_writes() {
    let fx = Fixture::new();
    let cue = assert_cmd::cargo::cargo_bin("cue");
    let marker = fx.env.temp_dir.path().join("nested-survived");
    fx.hook(
        "artifact_create.d",
        "10-nest",
        &format!(
            "#!/bin/sh\n[ \"$CUE_HOOK_DEPTH\" = 1 ] || {{ sleep 2; touch {}; exit 0; }}\n\"{}\" add inner body --type note --context work >/dev/null 2>&1\n",
            marker.display(),
            cue.display()
        ),
    );

    fx.cue()
        .env("CUE_HOOK_TIMEOUT_MS", "500")
        .env("CUE_HOOK_BUDGET_MS", "600")
        .args([
            "add",
            "outer",
            "body",
            "--type",
            "note",
            "--context",
            "work",
        ])
        .assert()
        .success();
    assert_eq!(fx.dispatch_log().last().unwrap()["outcome"], "timeout");

    std::thread::sleep(Duration::from_millis(2500));
    assert!(!marker.exists(), "nested hook escaped the outer timeout");
}
