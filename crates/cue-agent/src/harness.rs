//! The pi harness: how a run is spelled as argv, and how its stdout is read
//! back afterwards.
//!
//! Only pi is supported, and no `Harness` trait exists for it to implement.
//! Argv construction and line parsing live in their own functions instead, so
//! extracting a trait later is mechanical rather than speculative.

use crate::manifest::Agent;
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

/// Resolve the harness executable: explicit flag, then `$CUE_AGENT_HARNESS`,
/// then `pi` on `PATH`.
pub fn resolve(explicit: Option<PathBuf>) -> std::io::Result<PathBuf> {
    let path = explicit
        .or_else(|| std::env::var_os("CUE_AGENT_HARNESS").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("pi"));
    // Path-like values are relative to the caller, not --cwd. Keep bare names
    // intact for PATH lookup; missing executables remain isolated run failures.
    Ok(
        if path.is_relative() && path.as_os_str().as_encoded_bytes().contains(&b'/') {
            std::env::current_dir()?.join(path)
        } else {
            path
        },
    )
}

/// Ask the harness for its version, once per batch.
///
/// Best effort: a harness that cannot answer still runs, it is simply recorded
/// without a version.
pub fn version(harness: &Path, cwd: &Path) -> Option<String> {
    let mut output = tempfile::tempfile().ok()?;
    let mut child = Command::new(harness)
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
    if !agent.tools.is_empty() {
        args.push("--tools".to_string());
        args.push(agent.tools.join(","));
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
            tools: vec!["read".into(), "bash".into()],
            thinking: None,
            timeout_secs: None,
            source: Source::User,
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
        agent.tools.clear();
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

    #[test]
    fn a_missing_events_file_captures_nothing_rather_than_failing() {
        let capture = capture(Path::new("/nonexistent/events.jsonl"));
        assert_eq!(capture.turns, 0);
        assert!(capture.response.is_empty());
    }
}
