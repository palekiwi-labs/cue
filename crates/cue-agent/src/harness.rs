//! The pi harness: how a run is spelled as argv, and how its stdout is read
//! back afterwards.
//!
//! Only pi is supported, and no `Harness` trait exists for it to implement.
//! Argv construction and line parsing live in their own functions instead, so
//! extracting a trait later is mechanical rather than speculative.

use crate::manifest::Agent;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A single event line larger than this is skipped rather than buffered.
const MAX_LINE_BYTES: usize = 1 << 20;
/// Parsing stops after this much of the stream. File-backed capture has no
/// backpressure, so the cap lives here, on the reading side.
const MAX_TOTAL_BYTES: u64 = 32 << 20;
/// Longest reply to `--version` still treated as a version.
const MAX_VERSION_BYTES: usize = 120;

/// The harness, found through normal executable lookup. There is no override.
pub const PROGRAM: &str = "pi";

/// The environment overlay one task applies to the inherited environment.
pub type EnvOverlay = BTreeMap<String, Option<String>>;

/// Locate `pi` the way the task's own exec would: on its effective `PATH`
/// (the inherited value, unless the task overlay sets or removes it, and the
/// platform default search path when it is unset), with
/// empty and relative entries taken against the task's cwd, as they are
/// after the child changes directory.
///
/// Resolving here rather than in `exec` gives the version probe and the run
/// one and the same executable, and records which one it was.
pub fn locate(env: &EnvOverlay, cwd: &Path) -> Result<PathBuf> {
    let path = search_path(env, std::env::var_os("PATH"))?;
    std::env::split_paths(&path)
        .map(|dir| cwd.join(dir).join(PROGRAM))
        .find(|candidate| is_executable(candidate))
        .with_context(|| format!("could not find {PROGRAM} on this task's PATH"))
}

/// The directories to search: the task overlay's `PATH`, else the inherited
/// one. When neither yields a value, normal Unix lookup searches the
/// platform default, so this does too. Only resolution uses that default:
/// a removed `PATH` stays removed from the child's environment. An explicitly
/// empty `PATH` is a value, not an absence; its one empty entry is the cwd.
fn search_path(env: &EnvOverlay, inherited: Option<OsString>) -> Result<OsString> {
    let path = match env.get("PATH") {
        Some(Some(value)) => Some(OsString::from(value)),
        Some(None) => None,
        None => inherited,
    };
    match path {
        Some(path) => Ok(path),
        None => default_search_path(),
    }
}

/// The platform's default executable search path, `confstr(_CS_PATH)`.
fn default_search_path() -> Result<OsString> {
    use std::os::unix::ffi::OsStringExt;
    let unavailable = || {
        anyhow::anyhow!(
            "could not find {PROGRAM}: PATH is unset and the platform defines no default search path"
        )
    };
    // SAFETY: a null buffer with length 0 only asks for the required size,
    // including the terminating NUL; 0 means no value or an error.
    let len = unsafe { libc::confstr(libc::_CS_PATH, std::ptr::null_mut(), 0) };
    if len == 0 {
        return Err(unavailable());
    }
    let mut buf = vec![0u8; len];
    // SAFETY: `buf` provides exactly `len` writable bytes.
    let written = unsafe { libc::confstr(libc::_CS_PATH, buf.as_mut_ptr().cast(), len) };
    // The value cannot grow between calls; refuse rather than truncate if it
    // somehow did, or if the second call failed.
    if written == 0 || written > len {
        return Err(unavailable());
    }
    buf.truncate(written - 1);
    Ok(OsString::from_vec(buf))
}

/// A regular file (after following symlinks) that this process may execute,
/// judged by the kernel with the effective ids, as `execve` would. Mode bits
/// alone are not enough: an execute bit for group or other does not grant the
/// owner execute rights.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    if !std::fs::metadata(path).is_ok_and(|meta| meta.is_file()) {
        return false;
    }
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
    unsafe { libc::faccessat(libc::AT_FDCWD, path.as_ptr(), libc::X_OK, libc::AT_EACCESS) == 0 }
}

/// Ask the harness for its version with exactly the environment overlay and
/// cwd of the task that will run it; callers share an answer only between
/// tasks identical in executable, cwd and environment.
///
/// Best effort: a harness that cannot answer still runs, it is simply recorded
/// without a version.
pub fn version(harness: &Path, cwd: &Path, env: &EnvOverlay) -> Option<String> {
    let mut output = tempfile::tempfile().ok()?;
    let mut command = Command::new(harness);
    for (key, value) in env {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let mut child = command
        .arg("--version")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().ok()?))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Err(_) => break None,
            Ok(None) => {}
        }
        if started.elapsed() >= Duration::from_secs(2)
            || output
                .metadata()
                .map_or(true, |m| m.len() > (MAX_VERSION_BYTES + 1) as u64)
        {
            break None;
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    // Also stop in-group descendants after the leader exits. No pipe EOF wait.
    // SAFETY: the child was created as leader of its own process group.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.wait();
    if !status?.success() {
        return None;
    }
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut bytes = Vec::new();
    output
        .take((MAX_VERSION_BYTES + 2) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_VERSION_BYTES + 1 {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .next()
        .map(|line| line.trim().to_string())
        // A version string is a short line. Anything longer is some other
        // output, and copying it into every receipt and trace would be worse
        // than recording no version at all.
        .filter(|line| !line.is_empty() && line.len() <= MAX_VERSION_BYTES)
}

/// Build the argv for one run.
///
/// Mirrors pi's own subagent extension (`examples/extensions/subagent/
/// index.ts:300-341`) with `--session-id` added, because cue-agent assigns run
/// identity rather than adopting one pi generates.
///
/// The prompt is the positional message, passed verbatim, which is what makes
/// `prompt.md` and the argument byte-identical. pi's `@file` form is
/// deliberately not used: it wraps file contents in `<file name=...>` markup
/// (`src/cli/file-processor.ts:73-78`), so the agent would not receive the
/// prompt as written.
pub fn argv(
    run_id: &str,
    agent: &Agent,
    prompt: &str,
    system_prompt_path: Option<&Path>,
) -> Vec<String> {
    let mut args = vec![
        "--mode".to_string(),
        "json".to_string(),
        "-p".to_string(),
        "--no-session".to_string(),
        "--session-id".to_string(),
        run_id.to_string(),
    ];
    if let Some(model) = &agent.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    if let Some(thinking) = &agent.thinking {
        args.push("--thinking".to_string());
        args.push(thinking.clone());
    }
    // Unset tools leave pi's defaults alone; an explicit empty list disables
    // every tool, extension tools included.
    match agent.tools.as_deref() {
        None => {}
        Some([]) => args.push("--no-tools".to_string()),
        Some(tools) => {
            args.push("--tools".to_string());
            args.push(tools.join(","));
        }
    }
    if let Some(path) = system_prompt_path {
        args.push("--append-system-prompt".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    args.push(prompt.to_string());
    args
}

/// What one run's `events.jsonl` yielded.
#[derive(Debug, Default, Clone)]
pub struct Capture {
    pub response: String,
    pub turns: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost_usd: f64,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
    pub malformed_lines: u64,
    pub oversized_lines: u64,
    pub truncated: bool,
}

/// Read a finished run's events.
///
/// Called after the child is reaped, so end of file means end of stream. A line
/// that does not parse is counted and skipped: a subagent's transcript must not
/// be lost because one line was interrupted mid-write.
pub fn capture(path: &Path) -> Capture {
    let mut capture = Capture::default();
    let Ok(file) = std::fs::File::open(path) else {
        return capture;
    };
    let mut reader = BufReader::new(file);
    let mut consumed: u64 = 0;

    loop {
        match read_capped_line(&mut reader, MAX_LINE_BYTES) {
            Ok(None) => break,
            Ok(Some(Line::Oversized(bytes))) => {
                consumed += bytes;
                capture.oversized_lines += 1;
            }
            Ok(Some(Line::Text(line))) => {
                consumed += line.len() as u64;
                absorb_line(&mut capture, &line);
            }
            Err(_) => break,
        }
        if consumed >= MAX_TOTAL_BYTES {
            capture.truncated = true;
            break;
        }
    }
    capture
}

fn absorb_line(capture: &mut Capture, line: &str) {
    if line.trim().is_empty() {
        return;
    }
    let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
        capture.malformed_lines += 1;
        return;
    };

    match event.get("type").and_then(|value| value.as_str()) {
        Some("session") => {
            capture.session_id = event
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        Some("message_end") => {
            let Some(message) = event.get("message") else {
                return;
            };
            if message.get("role").and_then(|role| role.as_str()) != Some("assistant") {
                return;
            }
            capture.turns += 1;
            if let Some(usage) = message.get("usage") {
                capture.tokens_input += number(usage, "input");
                capture.tokens_output += number(usage, "output");
                capture.cache_read += number(usage, "cacheRead");
                capture.cache_write += number(usage, "cacheWrite");
                capture.cost_usd += usage
                    .get("cost")
                    .and_then(|cost| cost.get("total"))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
            }
            if capture.model.is_none() {
                capture.model = message
                    .get("model")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
            }
            capture.stop_reason = message
                .get("stopReason")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            capture.error_message = message
                .get("errorMessage")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            capture.response = assistant_text(message).unwrap_or_default();
        }
        _ => {}
    }
}

/// All text parts of an assistant message, in order, without added separators.
fn assistant_text(message: &serde_json::Value) -> Option<String> {
    let content = message.get("content")?.as_array()?;
    Some(
        content
            .iter()
            .filter(|part| part.get("type").and_then(|v| v.as_str()) == Some("text"))
            .filter_map(|part| part.get("text").and_then(|v| v.as_str()))
            .collect(),
    )
}

fn number(value: &serde_json::Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

enum Line {
    Text(String),
    /// A line past the cap; its length is reported, its bytes are discarded.
    Oversized(u64),
}

/// Read one line, discarding rather than buffering anything past `cap`.
///
/// A capped `read_line` would still allocate the whole line before rejecting
/// it, which is exactly the unbounded memory the cap exists to prevent.
fn read_capped_line<R: BufRead>(reader: &mut R, cap: usize) -> std::io::Result<Option<Line>> {
    let mut buf: Vec<u8> = Vec::new();
    let mut length: u64 = 0;
    let mut oversized = false;

    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        if available.is_empty() {
            break;
        }
        let (chunk, done) = match available.iter().position(|byte| *byte == b'\n') {
            Some(index) => (&available[..=index], true),
            None => (available, false),
        };
        length += chunk.len() as u64;
        if !oversized {
            if buf.len() + chunk.len() > cap {
                oversized = true;
                buf = Vec::new();
            } else {
                buf.extend_from_slice(chunk);
            }
        }
        let consumed = chunk.len();
        reader.consume(consumed);
        if done {
            break;
        }
    }

    if length == 0 {
        return Ok(None);
    }
    if oversized {
        return Ok(Some(Line::Oversized(length)));
    }
    Ok(Some(Line::Text(String::from_utf8_lossy(&buf).into_owned())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Agent, Source};

    #[test]
    fn capture_preserves_all_text_of_the_final_message_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, concat!(
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"stopReason\":\"error\",\"errorMessage\":\"old\",\"content\":[{\"type\":\"text\",\"text\":\"earlier\"}]}}\n",
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"first\\n\"},{\"type\":\"thinking\",\"thinking\":\"private\"},{\"type\":\"text\",\"text\":\"last\"}]}}\n"
        )).unwrap();
        let result = capture(&path);
        assert_eq!(result.response, "first\nlast");
        assert_eq!(result.turns, 2);
        assert_eq!(result.stop_reason, None);
        assert_eq!(result.error_message, None);
        use std::io::Write;
        writeln!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap(),
            r#"{{"type":"message_end","message":{{"role":"assistant","content":[]}}}}"#
        )
        .unwrap();
        assert!(capture(&path).response.is_empty());
    }

    fn agent() -> Agent {
        Agent {
            name: "explore".into(),
            description: None,
            model: Some("anthropic/haiku".into()),
            system_prompt: "You explore.".into(),
            tools: Some(vec!["read".into(), "bash".into()]),
            thinking: None,
            source: Source::User,
            field_sources: Default::default(),
        }
    }

    #[test]
    fn argv_carries_the_prompt_verbatim_as_the_positional_message() {
        let args = argv(
            "run-1",
            &agent(),
            "Find the bug",
            Some(Path::new("/tmp/sp.md")),
        );
        assert_eq!(
            args,
            vec![
                "--mode",
                "json",
                "-p",
                "--no-session",
                "--session-id",
                "run-1",
                "--model",
                "anthropic/haiku",
                "--tools",
                "read,bash",
                "--append-system-prompt",
                "/tmp/sp.md",
                "Find the bug",
            ]
        );
    }

    #[test]
    fn an_agent_without_a_model_or_tools_inherits_the_harness_defaults() {
        let mut agent = agent();
        agent.model = None;
        agent.tools = None;
        let args = argv("run-1", &agent, "hello", None);
        assert_eq!(
            args,
            vec![
                "--mode",
                "json",
                "-p",
                "--no-session",
                "--session-id",
                "run-1",
                "hello"
            ]
        );
    }

    #[test]
    fn an_explicitly_empty_tool_list_disables_every_tool() {
        let mut agent = agent();
        agent.model = None;
        agent.tools = Some(Vec::new());
        let args = argv("run-1", &agent, "hello", None);
        assert_eq!(&args[6..], ["--no-tools", "hello"]);
    }

    #[test]
    fn an_oversized_line_is_skipped_without_losing_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut giant = String::from("{\"type\":\"junk\",\"blob\":\"");
        giant.push_str(&"x".repeat(MAX_LINE_BYTES + 16));
        giant.push_str("\"}\n");
        let good = r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#;
        std::fs::write(&path, format!("{giant}{good}\n")).unwrap();

        let capture = capture(&path);
        assert_eq!(capture.oversized_lines, 1);
        assert_eq!(capture.response, "done");
    }

    #[test]
    fn malformed_lines_are_counted_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            "not json\n{\"type\":\"message_end\"\n{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}\n",
        )
        .unwrap();

        let capture = capture(&path);
        assert_eq!(capture.malformed_lines, 2);
        assert_eq!(capture.response, "ok");
        assert_eq!(capture.turns, 1);
    }

    fn executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn pi_is_located_on_the_tasks_own_path() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("cwd");
        executable(&dir.path().join("first/pi"));
        executable(&cwd.join("rel/pi"));
        executable(&cwd.join("pi"));
        std::fs::create_dir_all(dir.path().join("empty")).unwrap();
        // Not executable: skipped rather than chosen.
        std::fs::write(dir.path().join("empty/pi"), "").unwrap();

        let overlay =
            |path: &str| -> EnvOverlay { [("PATH".to_string(), Some(path.to_string()))].into() };
        let empty = dir.path().join("empty").display().to_string();
        let first = dir.path().join("first").display().to_string();

        assert_eq!(
            locate(&overlay(&format!("{empty}:{first}")), &cwd).unwrap(),
            dir.path().join("first/pi")
        );
        assert_eq!(
            locate(&overlay(&format!("{empty}:rel")), &cwd).unwrap(),
            cwd.join("rel/pi")
        );
        assert_eq!(
            locate(&overlay(&format!("{empty}::{first}")), &cwd).unwrap(),
            cwd.join("pi")
        );
        let missing = format!("{:#}", locate(&overlay(&empty), &cwd).unwrap_err());
        assert!(missing.contains("could not find pi"), "{missing}");
    }

    /// The platform's default search path, read independently of the code
    /// under test.
    fn confstr_path() -> OsString {
        use std::os::unix::ffi::OsStringExt;
        // SAFETY: a null buffer of length 0 only queries the needed size.
        let len = unsafe { libc::confstr(libc::_CS_PATH, std::ptr::null_mut(), 0) };
        assert!(len > 0, "this platform defines _CS_PATH");
        let mut buf = vec![0u8; len];
        // SAFETY: `buf` holds `len` writable bytes.
        unsafe { libc::confstr(libc::_CS_PATH, buf.as_mut_ptr().cast(), len) };
        buf.pop();
        OsString::from_vec(buf)
    }

    #[test]
    fn the_search_path_follows_the_overlay_then_the_inherited_value() {
        let inherited = Some(OsString::from("/inherited"));
        let none = EnvOverlay::new();
        assert_eq!(search_path(&none, inherited.clone()).unwrap(), "/inherited");
        let set: EnvOverlay = [("PATH".to_string(), Some("/task".to_string()))].into();
        assert_eq!(search_path(&set, inherited.clone()).unwrap(), "/task");
        // An explicitly empty PATH is kept: its one empty entry is the cwd.
        let empty: EnvOverlay = [("PATH".to_string(), Some(String::new()))].into();
        assert_eq!(search_path(&empty, inherited.clone()).unwrap(), "");
    }

    #[test]
    fn an_unset_path_searches_the_platform_default() {
        let default = confstr_path();
        assert!(!default.is_empty());
        // Removed by the task, though the supervisor has one.
        let removed: EnvOverlay = [("PATH".to_string(), None)].into();
        assert_eq!(
            search_path(&removed, Some(OsString::from("/inherited"))).unwrap(),
            default
        );
        // Absent from the supervisor and untouched by the task.
        assert_eq!(search_path(&EnvOverlay::new(), None).unwrap(), default);
    }

    #[test]
    fn a_candidate_the_current_user_cannot_execute_is_skipped() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_path_buf();
        // Group and other may execute it, but its owner (this user) may not:
        // a mode-bit check accepts it, an effective-access check does not.
        let denied = dir.path().join("denied/pi");
        executable(&denied);
        std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o611)).unwrap();
        // An executable directory named pi is not a program.
        std::fs::create_dir_all(dir.path().join("dir/pi")).unwrap();
        executable(&dir.path().join("good/pi"));
        let path = ["dir", "denied", "good"]
            .map(|sub| dir.path().join(sub).display().to_string())
            .join(":");
        let env: EnvOverlay = [("PATH".to_string(), Some(path))].into();
        let found = locate(&env, &cwd).unwrap();
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            // Root may execute any file with some execute bit set, so the
            // denied candidate is genuinely runnable there.
            assert_eq!(found, denied);
        } else {
            assert_eq!(found, dir.path().join("good/pi"));
        }
    }

    #[test]
    fn a_missing_events_file_captures_nothing_rather_than_failing() {
        let capture = capture(Path::new("/nonexistent/events.jsonl"));
        assert_eq!(capture.turns, 0);
        assert!(capture.response.is_empty());
    }
}
