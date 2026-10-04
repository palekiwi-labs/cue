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
/// directory and a fake `pi` first on `PATH`.
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
        for sub in ["state", "config", "project", "harness", "store", "bin"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect("sandbox subdir");
        }
        let sandbox = Self { dir };
        write_executable(&sandbox.harness(), FAKE_HARNESS);
        write_executable(&sandbox.pi_dir().join("pi"), FAKE_HARNESS);
        sandbox
    }

    /// The directory holding the `pi` that normal executable lookup finds.
    pub fn pi_dir(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    /// Replace the `pi` on the sandbox `PATH` with another script.
    pub fn install_pi(&self, body: &str) {
        write_executable(&self.pi_dir().join("pi"), body);
    }

    /// A `PATH` value that puts `dir` first, ahead of the inherited entries
    /// (which supply `bash` and `env` for the fake scripts). The sandbox's own
    /// `pi` directory is deliberately absent.
    pub fn path_with(&self, dir: &Path) -> String {
        let mut paths = vec![dir.to_path_buf()];
        if let Some(path) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&path));
        }
        std::env::join_paths(paths)
            .expect("PATH")
            .to_string_lossy()
            .into_owned()
    }

    /// Create an existing context in the sandbox store.
    pub fn context(&self, address: &str) {
        let dir = self.store().join(address);
        std::fs::create_dir_all(&dir).expect("context dir");
        std::fs::write(dir.join("context.md"), "---\n---\n").expect("context.md");
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

    /// Write the global agent manifest (`$XDG_CONFIG_HOME/cue/cue-agent.json`).
    pub fn global_manifest(&self, json: &str) {
        let dir = self.config().join("cue");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("cue-agent.json"), json).expect("global manifest");
    }

    /// Write the project-local manifest (`cue-agent.json` in the project).
    pub fn local_manifest(&self, json: &str) {
        std::fs::write(self.project().join("cue-agent.json"), json).expect("local manifest");
    }

    /// The fake harness outside `PATH`, for wrappers that delegate to it.
    pub fn harness(&self) -> PathBuf {
        self.dir.path().join("fake-pi")
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

    /// A `cue-agent` invocation wired to this sandbox. `pi` is found through
    /// normal executable lookup, with the sandbox's fake first on `PATH`.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::new(bin());
        cmd.current_dir(self.project())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.config())
            .env("CUE_STORE", self.store())
            .env("PATH", self.path_with(&self.pi_dir()))
            .env("FAKE_HARNESS_LOG", self.harness_log())
            .env_remove("CUE_CONTEXT");
        cmd
    }

    /// `cue-agent run --json <spec>` with the specification inline.
    pub fn run_json(&self, spec: &serde_json::Value) -> std::process::Output {
        self.cmd()
            .args(["run", "--json"])
            .arg(spec.to_string())
            .output()
            .expect("run cue-agent")
    }

    /// The environment one run's harness saw, as `KEY=VALUE` lines.
    pub fn recorded_env(&self, session_id: &str) -> std::collections::BTreeMap<String, String> {
        let path = self.harness_log().join(format!("{session_id}.env"));
        let raw = std::fs::read(&path)
            .unwrap_or_else(|err| panic!("missing env record {}: {err}", path.display()));
        raw.split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .filter_map(|entry| {
                let entry = String::from_utf8_lossy(entry);
                let (key, value) = entry.split_once('=')?;
                Some((key.to_string(), value.to_string()))
            })
            .collect()
    }

    /// The working directory one run's harness started in.
    pub fn recorded_cwd(&self, session_id: &str) -> String {
        std::fs::read_to_string(self.harness_log().join(format!("{session_id}.cwd")))
            .expect("cwd record")
            .trim()
            .to_string()
    }

    /// Assert that a refused request left no trace of having run.
    pub fn assert_nothing_ran(&self, output: &std::process::Output) {
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            !self.harness_log().join("cue.argv").exists(),
            "no trace may be written"
        );
        assert!(self.recorded_sessions().is_empty(), "no harness may start");
        assert!(
            !self.harness_log().join("version-probed").exists(),
            "the harness version is not probed for a refused request"
        );
        assert!(
            !self.state().join("cue").exists(),
            "no run state may be created"
        );
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

/// Run git in `dir` with configuration isolated from the host, returning
/// trimmed stdout; panics on failure.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_command(dir).args(args).output().expect("git");
    assert!(
        output.status.success(),
        "git {args:?} in {dir:?}: {output:?}"
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A git command isolated from the host's global and system configuration.
pub fn git_command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    command
}

/// A disposable repository at `dir` on branch `main`, with a committed
/// `README.md` (holding `readme`) and `sub/.keep`.
pub fn init_repo(dir: &Path, readme: &str) {
    std::fs::create_dir_all(dir.join("sub")).expect("repo dir");
    git(dir, &["init", "--quiet", "--initial-branch=main"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), readme).expect("readme");
    std::fs::write(dir.join("sub").join(".keep"), "").expect("keep");
    git(dir, &["add", "."]);
    git(dir, &["commit", "--quiet", "-m", "root"]);
}

/// Local branch names of the repository at `dir`, sorted.
pub fn branches(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = git(
        dir,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .lines()
    .map(str::to_string)
    .collect();
    names.sort();
    names
}

/// Checkout paths Git has registered for the repository at `dir`, sorted.
pub fn worktrees(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = git(dir, &["worktree", "list", "--porcelain"])
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect();
    paths.sort();
    paths
}

/// `paths`, sorted, for comparison with [`worktrees`].
pub fn sorted(paths: &[&Path]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = paths.iter().map(|path| path.to_path_buf()).collect();
    paths.sort();
    paths
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

/// A run specification from `(agent, prompt)` pairs.
pub fn spec(tasks: &[(&str, &str)]) -> serde_json::Value {
    let tasks: Vec<serde_json::Value> = tasks
        .iter()
        .map(|(agent, prompt)| serde_json::json!({ "agent": agent, "prompt": prompt }))
        .collect();
    serde_json::json!({ "tasks": tasks })
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
  if [[ -n "${FAKE_HARNESS_LOG:-}" ]]; then
    mkdir -p "$FAKE_HARNESS_LOG"
    : >"$FAKE_HARNESS_LOG/version-probed"
  fi
  printf '%s\n' "${FAKE_PI_VERSION:-fake-pi 9.9.9}"
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
  env -0 >"$FAKE_HARNESS_LOG/$session.env"
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
  *DIRTY*)
    # Leave tracked modifications and untracked files in the checkout.
    emit_session
    printf 'changed\n' >README.md
    printf 'new\n' >untracked.txt
    emit_message "dirtied ${PWD}"
    ;;
  *BREAK_GIT*)
    # Corrupt the checkout's .git link so Git refuses to remove it.
    emit_session
    printf 'gitdir: /nonexistent\n' >.git
    emit_message "broke the checkout"
    ;;
  *COMMIT*)
    # Commit in the checkout, so its final HEAD differs from the base.
    emit_session
    printf 'work\n' >committed.txt
    git add committed.txt && git commit --quiet -m work
    emit_message "committed $(git rev-parse HEAD)"
    ;;
  *)
    emit_session
    emit_message "echo: ${prompt}"
    ;;
esac
"#;
