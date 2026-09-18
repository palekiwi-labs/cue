//! Shared scaffolding for the `cue-agent` integration tests.
//!
//! Every test drives the real binary across a real process boundary: the
//! harness under test is a script on disk, not an injected trait, because the
//! behaviours that matter (process groups, inherited stdout, file-backed
//! capture) only exist at that boundary.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// A disposable machine: its own state directory, config directory, project
/// directory and fake harness.
pub struct Sandbox {
    pub dir: tempfile::TempDir,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        for sub in ["state", "config", "project", "harness", "store"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("sandbox subdir");
        }
        Self { dir }
    }

    pub fn state(&self) -> PathBuf {
        self.dir.path().join("state")
    }

    pub fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    pub fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    pub fn store(&self) -> PathBuf {
        self.dir.path().join("store")
    }

    /// Directory the fake harness uses to record what it was invoked with.
    pub fn harness_log(&self) -> PathBuf {
        self.dir.path().join("harness")
    }

    /// Write the global agent manifest (`$XDG_CONFIG_HOME/cue/agents.json`).
    pub fn global_manifest(&self, json: &str) {
        let dir = self.config().join("cue");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("agents.json"), json).expect("global manifest");
    }

    /// Write the project-local manifest (`.cue-agent.json` in the project).
    pub fn local_manifest(&self, json: &str) {
        std::fs::write(self.project().join(".cue-agent.json"), json).expect("local manifest");
    }

    /// Install the fake harness and return its path.
    pub fn harness(&self) -> PathBuf {
        let path = self.dir.path().join("fake-pi");
        write_executable(&path, FAKE_HARNESS);
        path
    }

    /// Install a fake `cue` binary that records the arguments cue-agent hands
    /// it, and return its path.
    pub fn fake_cue(&self) -> PathBuf {
        let path = self.dir.path().join("fake-cue");
        write_executable(
            &path,
            &FAKE_CUE.replace("__LOG__", &self.harness_log().to_string_lossy()),
        );
        path
    }

    /// The arguments the fake `cue` binary was called with, one per line.
    pub fn recorded_cue_argv(&self) -> Vec<String> {
        let raw = std::fs::read_to_string(self.harness_log().join("cue.argv")).expect("cue argv");
        raw.lines().map(str::to_string).collect()
    }

    /// Make the project a git repository with an origin remote and one commit,
    /// which is what cue needs to resolve a scope and stamp a trace.
    pub fn init_git_repo(&self, origin: &str) {
        let project = self.project();
        for args in [
            vec!["init", "--initial-branch=main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
            vec!["remote", "add", "origin", origin],
            vec!["commit", "--allow-empty", "-m", "root"],
        ] {
            let status = Command::new("git")
                .args(&args)
                .current_dir(&project)
                .output()
                .expect("git");
            assert!(status.status.success(), "git {args:?}: {status:?}");
        }
    }

    /// A `cue-agent` invocation wired to this sandbox.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::new(bin());
        cmd.current_dir(self.project())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.config())
            .env("CUE_STORE", self.store())
            .env("CUE_AGENT_HARNESS", self.harness())
            .env("FAKE_HARNESS_LOG", self.harness_log())
            .env_remove("CUE_CONTEXT");
        cmd
    }

    /// Recorded argv for one session id, one argument per line.
    pub fn recorded_argv(&self, session_id: &str) -> Vec<String> {
        let path = self.harness_log().join(format!("{session_id}.argv"));
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("missing argv record {}: {err}", path.display()));
        raw.lines().map(str::to_string).collect()
    }

    /// Every argv record written by the fake harness, sorted by session id.
    pub fn recorded_sessions(&self) -> Vec<String> {
        let mut ids: Vec<String> = std::fs::read_dir(self.harness_log())
            .expect("harness log dir")
            .filter_map(|entry| {
                let name = entry.ok()?.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".argv").map(str::to_string)
            })
            .collect();
        ids.sort();
        ids
    }
}

pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cue-agent"))
}

/// The real `cue` binary, built on demand.
///
/// `CARGO_BIN_EXE_*` only covers the crate's own binaries, and the trace plane
/// is defined by what `cue add` actually writes, so the sibling binary is built
/// rather than faked for the end-to-end case.
pub fn real_cue() -> PathBuf {
    let sibling = bin().parent().expect("target dir").join("cue");
    if sibling.is_file() {
        return sibling;
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "cue", "--quiet"])
        .current_dir(&workspace)
        .status()
        .expect("cargo build -p cue");
    assert!(status.success(), "could not build the cue binary");
    assert!(sibling.is_file(), "cue binary missing at {sibling:?}");
    sibling
}

pub fn write_executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).expect("write script");
    let mut perms = std::fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod");
}

/// Parse the single JSON receipt `cue-agent` prints on stdout.
pub fn receipt(stdout: &[u8]) -> serde_json::Value {
    serde_json::from_slice(stdout).unwrap_or_else(|err| {
        panic!(
            "stdout was not a JSON receipt: {err}\n---\n{}",
            String::from_utf8_lossy(stdout)
        )
    })
}

/// Look up one run receipt by agent name.
pub fn run_of<'a>(receipt: &'a serde_json::Value, agent: &str) -> &'a serde_json::Value {
    receipt["runs"]
        .as_array()
        .expect("runs array")
        .iter()
        .find(|run| run["agent"] == agent)
        .unwrap_or_else(|| panic!("no run for agent {agent}"))
}

/// A fake `cue` binary: records its argv and the body it was given, then
/// reports success.
pub const FAKE_CUE: &str = r#"#!/usr/bin/env bash
set -u
log="__LOG__"
mkdir -p "$log"
: >"$log/cue.argv"
for arg in "$@"; do
  printf '%s\n' "$arg" >>"$log/cue.argv"
done
for ((i = 1; i <= $#; i++)); do
  if [[ "${!i}" == "--file" ]]; then
    j=$((i + 1))
    cp "${!j}" "$log/cue.body"
  fi
done
printf 'trace\n'
"#;

/// The fake harness.
///
/// It behaves like `pi --mode json -p`: it emits JSONL events on stdout and
/// diagnostics on stderr. Behaviour is selected by directives embedded in the
/// prompt so that one script can serve every scenario, and each invocation
/// records its own argv keyed by the session id it was given.
pub const FAKE_HARNESS: &str = r#"#!/usr/bin/env bash
# Fake pi harness. Records argv, then acts on directives in the prompt.
set -u

if [[ "${1:-}" == "--version" ]]; then
  printf 'fake-pi 9.9.9\n'
  exit 0
fi

args=("$@")
session=""
prompt=""
for ((i = 0; i < ${#args[@]}; i++)); do
  case "${args[i]}" in
    --session-id) session="${args[i + 1]}" ;;
  esac
done
prompt="${args[${#args[@]} - 1]}"

if [[ -n "${FAKE_HARNESS_LOG:-}" ]]; then
  mkdir -p "$FAKE_HARNESS_LOG"
  : >"$FAKE_HARNESS_LOG/$session.argv"
  for arg in "${args[@]}"; do
    printf '%s\n' "$arg" >>"$FAKE_HARNESS_LOG/$session.argv"
  done
  printf '%s\n' "${CUE_CONTEXT-<unset>}" >"$FAKE_HARNESS_LOG/$session.context"
  printf '%s\n' "$PWD" >"$FAKE_HARNESS_LOG/$session.cwd"
  # Concurrency witness: hold a marker for the lifetime of the process.
  mkdir -p "$FAKE_HARNESS_LOG/live"
  : >"$FAKE_HARNESS_LOG/live/$session"
  live=$(ls "$FAKE_HARNESS_LOG/live" | wc -l)
  printf '%s\n' "$live" >>"$FAKE_HARNESS_LOG/concurrency"
fi

emit_session() {
  printf '{"type":"session","version":3,"id":"%s"}\n' "$session"
}

emit_message() {
  printf '{"type":"message_end","message":{"role":"assistant","model":"fake","stopReason":"stop","content":[{"type":"text","text":"%s"}],"usage":{"input":11,"output":22,"cacheRead":0,"cacheWrite":0,"totalTokens":33,"cost":{"input":0.01,"output":0.02,"cacheRead":0,"cacheWrite":0,"total":0.03}}}}\n' "$1"
}

finish() {
  rm -f "${FAKE_HARNESS_LOG:-/nonexistent}/live/$session" 2>/dev/null
}
trap finish EXIT

case "$prompt" in
  *BLOCK_RECEIPT*)
    for ((i = 0; i < ${#args[@]}; i++)); do
      if [[ "${args[i]}" == "--append-system-prompt" ]]; then
        mkdir "$(dirname "${args[i + 1]}")/receipt.json"
      fi
    done
    emit_message "result survives"
    ;;
  *SLEEP=*)
    secs="${prompt##*SLEEP=}"
    secs="${secs%% *}"
    emit_session
    sleep "$secs"
    emit_message "slept ${secs}"
    ;;
  *IGNORE_TERM=*)
    # Ignore SIGTERM outright, so only SIGKILL ends this process.
    secs="${prompt##*IGNORE_TERM=}"
    secs="${secs%% *}"
    trap '' TERM
    : >"$FAKE_HARNESS_LOG/$session.ready"
    emit_session
    end=$((SECONDS + secs))
    while ((SECONDS < end)); do sleep 0.2; done
    emit_message "ignored term"
    ;;
  *ORPHAN=*)
    # A grandchild that outlives its parent while holding the inherited
    # stdout. With pipes this hangs the supervisor; with files it must not.
    secs="${prompt##*ORPHAN=}"
    secs="${secs%% *}"
    emit_session
    setsid bash -c "sleep $secs; printf '{\"type\":\"orphan\"}\n'" &
    emit_message "spawned orphan"
    ;;
  *FAIL=*)
    code="${prompt##*FAIL=}"
    code="${code%% *}"
    emit_session
    printf 'fake harness exploded\n' >&2
    exit "$code"
    ;;
  *MALFORMED*)
    emit_session
    printf 'not json at all\n'
    printf '{"type":"message_end","message":{"role":"assistant"\n'
    emit_message "survived malformed lines"
    ;;
  *OVERSIZED*)
    emit_session
    printf '{"type":"junk","blob":"'
    head -c 2000000 /dev/zero | tr '\0' 'x'
    printf '"}\n'
    emit_message "survived an oversized line"
    ;;
  *STDIN*)
    # Reads standard input to completion: with /dev/null this returns at once,
    # with an open pipe it would hang until the parent closed the write end.
    emit_session
    input="$(cat)"
    emit_message "stdin bytes:${#input}"
    ;;
  *SILENT*)
    emit_session
    ;;
  *)
    emit_session
    emit_message "echo: ${prompt}"
    ;;
esac
"#;
