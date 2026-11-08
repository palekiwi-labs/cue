//! How a run is spelled as argv, and how its stdout is read back afterwards.
//!
//! Two harnesses are supported: pi and claude-code. There is no `Harness`
//! trait; argv construction, executable lookup and line parsing dispatch on
//! the `Harness` enum, and both parsers fill the same `Capture` struct so
//! supervision, receipts, traces and monitoring stay harness-agnostic.
//! cue-agent's API is modeled on pi; the claude-code side is a translation,
//! never a second API.

use crate::manifest::Agent;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The harness a named agent runs in, from the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Harness {
    Pi,
    ClaudeCode,
}

impl Harness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::ClaudeCode => "claude-code",
        }
    }

    /// The executable name found through normal executable lookup.
    pub fn program(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::ClaudeCode => "claude",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pi" => Ok(Self::Pi),
            "claude-code" => Ok(Self::ClaudeCode),
            other => bail!("unknown harness '{other}'; expected 'pi' or 'claude-code'"),
        }
    }

    /// The harness a stored record names, when it names one this version
    /// knows. Records predate the field, and future names may appear.
    pub fn try_name(value: &str) -> Option<Self> {
        match value {
            "pi" => Some(Self::Pi),
            "claude-code" => Some(Self::ClaudeCode),
            _ => None,
        }
    }
}

/// A single event line larger than this is skipped rather than buffered.
const MAX_LINE_BYTES: usize = 1 << 20;
/// Parsing stops after this much of the stream. File-backed capture has no
/// backpressure, so the cap lives here, on the reading side.
const MAX_TOTAL_BYTES: u64 = 32 << 20;
/// Longest reply to `--version` still treated as a version.
const MAX_VERSION_BYTES: usize = 120;

/// The environment overlay one task applies to the inherited environment.
pub type EnvOverlay = BTreeMap<String, Option<String>>;

/// Locate the harness program the way the task's own exec would: on its
/// effective `PATH` (the inherited value, unless the task overlay sets or
/// removes it, and the platform default search path when it is unset), with
/// empty and relative entries taken against the task's cwd, as they are
/// after the child changes directory.
///
/// Resolving here rather than in `exec` gives the version probe and the run
/// one and the same executable, and records which one it was.
pub fn locate(harness: Harness, env: &EnvOverlay, cwd: &Path) -> Result<PathBuf> {
    let program = harness.program();
    let path = search_path(env, std::env::var_os("PATH"))
        .with_context(|| format!("could not find {program}"))?;
    std::env::split_paths(&path)
        .map(|dir| cwd.join(dir).join(program))
        .find(|candidate| is_executable(candidate))
        .with_context(|| format!("could not find {program} on this task's PATH"))
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
    let unavailable =
        || anyhow::anyhow!("PATH is unset and the platform defines no default search path");
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
    harness: Harness,
    run_id: &str,
    agent: &Agent,
    prompt: &str,
    system_prompt_path: Option<&Path>,
) -> Vec<String> {
    match harness {
        Harness::Pi => pi_argv(run_id, agent, prompt, system_prompt_path),
        Harness::ClaudeCode => claude_argv(agent, prompt, system_prompt_path),
    }
}

fn pi_argv(
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

/// The claude-code headless translation of the same pi-shaped fields.
///
/// Like pi, claude-code streams its events to stdout, so the same file-backed
/// capture applies. Unlike pi, its `--session-id` demands a UUID, so none is
/// passed: the session id it emits in the stream is recorded instead.
/// Tools pre-approved for a claude-code agent whose manifest leaves `tools`
/// unset.
const CLAUDE_DEFAULT_ALLOWED_TOOLS: &str = "Read,Edit,Write,Bash";

fn claude_argv(agent: &Agent, prompt: &str, system_prompt_path: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        // A headless run cannot answer a prompt, and `dontAsk` is the closest
        // match for pi, which never prompts.
        "--permission-mode".to_string(),
        "dontAsk".to_string(),
    ];
    if let Some(model) = &agent.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    // pi's thinking levels low..max are claude-code's effort levels verbatim;
    // `off` and `minimal` have no counterpart and are dropped, not rejected.
    if let Some(thinking) = agent.thinking.as_deref()
        && matches!(thinking, "low" | "medium" | "high" | "xhigh" | "max")
    {
        args.push("--effort".to_string());
        args.push(thinking.to_string());
    }
    // --tools restricts which tools exist; --allowedTools pre-approves them,
    // which dontAsk otherwise denies. Unset tools keep claude's tool set and
    // pre-approve the core coding tools, so the agent can edit and run
    // commands as a pi agent can.
    match agent.tools.as_deref() {
        None => {
            args.push("--allowedTools".to_string());
            args.push(CLAUDE_DEFAULT_ALLOWED_TOOLS.to_string());
        }
        Some([]) => {
            args.push("--tools".to_string());
            args.push(String::new());
        }
        Some(tools) => {
            let tools = tools.join(",");
            args.push("--tools".to_string());
            args.push(tools.clone());
            args.push("--allowedTools".to_string());
            args.push(tools);
        }
    }
    if let Some(path) = system_prompt_path {
        args.push("--append-system-prompt-file".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    // --tools and --allowedTools are variadic: without the terminator they
    // swallow the prompt, and a prompt starting with '-' would be an option.
    args.push("--".to_string());
    args.push(prompt.to_string());
    args
}

/// What one run's `events.jsonl` yielded.
#[derive(Debug, Default, Clone)]
pub struct Capture {
    /// Text of the last assistant message, when that message made no tool
    /// call.
    pub response: String,
    /// Text of the last assistant message, when that message made a tool
    /// call. Whether it is the answer depends on how the run ended.
    pub tool_call_response: Option<String>,
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
    /// The most recent tool call, for observing a run in progress.
    pub last_tool: Option<ToolCall>,
    /// The stream itself reported the run as failed, whatever the exit
    /// status says: pi's error/aborted stop reasons, claude-code's terminal
    /// result event carrying is_error.
    pub is_error: bool,
}

/// A tool call as the event stream shows it.
#[derive(Debug, Default, Clone)]
pub struct ToolCall {
    pub id: Option<String>,
    pub name: String,
    /// The arguments as compact JSON.
    pub arguments: String,
    /// Requested or started, with no result seen yet.
    pub pending: bool,
}

impl Capture {
    /// The run's final response, given whether it completed.
    ///
    /// A message that makes tool calls is normally followed by another turn,
    /// but a tool can end the run instead, and then that message is the last
    /// thing the model said. Only a completed run shows that no turn was cut
    /// off, so only it may promote such a message.
    pub fn final_response(&self, completed: bool) -> String {
        match &self.tool_call_response {
            Some(text) if completed => text.clone(),
            _ => self.response.clone(),
        }
    }
}

/// Read a run's events, finished or not: a live file only has a possibly
/// partial last line, which counts as malformed.
///
/// Called after the child is reaped, so end of file means end of stream. A line
/// that does not parse is counted and skipped: a subagent's transcript must not
/// be lost because one line was interrupted mid-write.
pub fn capture(harness: Harness, path: &Path) -> Capture {
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
                match harness {
                    Harness::Pi => absorb_pi_line(&mut capture, &line),
                    Harness::ClaudeCode => absorb_claude_line(&mut capture, &line),
                }
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

fn absorb_pi_line(capture: &mut Capture, line: &str) {
    if line.trim().is_empty() {
        return;
    }
    let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
        capture.malformed_lines += 1;
        return;
    };

    let text_of = |key: &str| {
        event
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
    };
    match event.get("type").and_then(|value| value.as_str()) {
        Some("tool_execution_start") => {
            capture.last_tool = Some(ToolCall {
                id: text_of("toolCallId"),
                name: text_of("toolName").unwrap_or_default(),
                arguments: event
                    .get("args")
                    .map(serde_json::Value::to_string)
                    .unwrap_or_default(),
                pending: true,
            });
        }
        Some("tool_execution_end") => {
            if let Some(tool) = &mut capture.last_tool
                && tool.id == text_of("toolCallId")
            {
                tool.pending = false;
            }
        }
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
            capture.is_error = matches!(
                capture.stop_reason.as_deref(),
                Some("error") | Some("aborted")
            );
            // pi continues after any message carrying tool calls, whatever its
            // stop reason, unless a tool terminates the run. Which happened is
            // known only from how the run ended, so the text is held apart.
            if let Some(call) = last_tool_call(message) {
                capture.last_tool = Some(call);
            }
            let text = assistant_text(message).unwrap_or_default();
            if has_tool_call(message) || capture.stop_reason.as_deref() == Some("toolUse") {
                capture.response = String::new();
                capture.tool_call_response = Some(text);
            } else {
                capture.response = text;
                capture.tool_call_response = None;
            }
        }
        _ => {}
    }
}

/// One claude-code `stream-json` line (shapes seen from claude-code 2.1.306).
///
/// The `result` event is the source of truth: it carries the final text,
/// failure, cost, turns and token totals. A run that spawns background
/// sub-agents emits more than one; the last one wins. Assistant events only
/// feed the live monitor and a fallback response for a run killed before any
/// result. Events of sub-agents (`parent_tool_use_id` set) are ignored.
fn absorb_claude_line(capture: &mut Capture, line: &str) {
    if line.trim().is_empty() {
        return;
    }
    let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
        capture.malformed_lines += 1;
        return;
    };
    let from_subagent = event
        .get("parent_tool_use_id")
        .is_some_and(|id| !id.is_null());
    match event.get("type").and_then(|value| value.as_str()) {
        Some("system") => {
            if event.get("subtype").and_then(|v| v.as_str()) == Some("init") {
                capture.session_id = event
                    .get("session_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                if capture.model.is_none() {
                    capture.model = event
                        .get("model")
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                }
            }
        }
        Some("assistant") if !from_subagent => {
            let Some(message) = event.get("message") else {
                return;
            };
            // A synthetic error message carries the failure in its text; a
            // later result event overrides it.
            if event.get("is_api_error_message").and_then(|v| v.as_bool()) == Some(true) {
                capture.is_error = true;
                capture.error_message = assistant_text(message).filter(|text| !text.is_empty());
            }
            if let Some(call) = last_claude_tool_use(message) {
                capture.last_tool = Some(call);
            }
            if let Some(text) = assistant_text(message).filter(|text| !text.is_empty()) {
                capture.response = text;
            }
        }
        Some("user") if !from_subagent => {
            let Some(content) = event
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(|content| content.as_array())
            else {
                return;
            };
            for part in content {
                if part.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                    continue;
                }
                if let Some(tool) = &mut capture.last_tool
                    && tool.id.as_deref() == part.get("tool_use_id").and_then(|v| v.as_str())
                {
                    tool.pending = false;
                }
            }
        }
        Some("result") => {
            // claude-code exits 0 even when the run failed, so is_error must
            // be honoured over the exit status.
            capture.is_error = event.get("is_error").and_then(|v| v.as_bool()) == Some(true);
            let text = event
                .get("result")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if capture.is_error {
                capture.error_message = text.or_else(|| {
                    event
                        .get("terminal_reason")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                });
                capture.response = String::new();
            } else {
                capture.error_message = None;
                capture.response = text.unwrap_or_default();
            }
            capture.tool_call_response = None;
            capture.stop_reason = event
                .get("stop_reason")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            capture.turns = event.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0);
            capture.cost_usd = event
                .get("total_cost_usd")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            // modelUsage is cumulative over the whole process, sub-agents
            // included, and keyed by model.
            let models = event.get("modelUsage").and_then(|v| v.as_object());
            let total = |key: &str| models.map_or(0, |m| m.values().map(|u| number(u, key)).sum());
            capture.tokens_input = total("inputTokens");
            capture.tokens_output = total("outputTokens");
            capture.cache_read = total("cacheReadInputTokens");
            capture.cache_write = total("cacheCreationInputTokens");
        }
        _ => {}
    }
}

/// The last `tool_use` block of a claude-code assistant message, pending
/// until the matching `tool_result` arrives.
fn last_claude_tool_use(message: &serde_json::Value) -> Option<ToolCall> {
    let part = message
        .get("content")?
        .as_array()?
        .iter()
        .rfind(|part| part.get("type").and_then(|v| v.as_str()) == Some("tool_use"))?;
    let text = |key: &str| part.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Some(ToolCall {
        id: text("id"),
        name: text("name").unwrap_or_default(),
        arguments: part
            .get("input")
            .map(serde_json::Value::to_string)
            .unwrap_or_default(),
        pending: true,
    })
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

/// The last tool call a message requests, pending until its execution ends.
fn last_tool_call(message: &serde_json::Value) -> Option<ToolCall> {
    let part = message
        .get("content")?
        .as_array()?
        .iter()
        .rfind(|part| part.get("type").and_then(|v| v.as_str()) == Some("toolCall"))?;
    let text = |key: &str| part.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Some(ToolCall {
        id: text("id"),
        name: text("name").unwrap_or_default(),
        arguments: part
            .get("arguments")
            .map(serde_json::Value::to_string)
            .unwrap_or_default(),
        pending: true,
    })
}

fn has_tool_call(message: &serde_json::Value) -> bool {
    message
        .get("content")
        .and_then(|content| content.as_array())
        .is_some_and(|parts| {
            parts
                .iter()
                .any(|part| part.get("type").and_then(|v| v.as_str()) == Some("toolCall"))
        })
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
    fn the_last_tool_call_is_pending_until_its_execution_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"toolCall\",\"id\":\"c1\",\"name\":\"read\",\"arguments\":{\"path\":\"a\"}}]}}\n").unwrap();
        let tool = capture(Harness::Pi, &path).last_tool.unwrap();
        assert_eq!((tool.name.as_str(), tool.pending), ("read", true));
        assert_eq!(tool.arguments, r#"{"path":"a"}"#);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        file.write_all(concat!(
            "{\"type\":\"tool_execution_start\",\"toolCallId\":\"c1\",\"toolName\":\"read\",\"args\":{\"path\":\"a\"}}\n",
            "{\"type\":\"tool_execution_end\",\"toolCallId\":\"c0\",\"toolName\":\"read\",\"isError\":false}\n",
        ).as_bytes()).unwrap();
        assert!(
            capture(Harness::Pi, &path).last_tool.unwrap().pending,
            "another call's result"
        );
        file.write_all(
            b"{\"type\":\"tool_execution_end\",\"toolCallId\":\"c1\",\"toolName\":\"read\",\"isError\":false}\n",
        ).unwrap();
        assert!(!capture(Harness::Pi, &path).last_tool.unwrap().pending);
    }

    #[test]
    fn capture_preserves_all_text_of_the_final_message_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, concat!(
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"stopReason\":\"error\",\"errorMessage\":\"old\",\"content\":[{\"type\":\"text\",\"text\":\"earlier\"}]}}\n",
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"first\\n\"},{\"type\":\"thinking\",\"thinking\":\"private\"},{\"type\":\"text\",\"text\":\"last\"}]}}\n"
        )).unwrap();
        let result = capture(Harness::Pi, &path);
        assert_eq!(result.response, "first\nlast");
        assert_eq!(result.turns, 2);
        assert_eq!(result.stop_reason, None);
        assert_eq!(result.error_message, None);
        assert!(!result.is_error, "a later success clears an earlier error");
        use std::io::Write;
        writeln!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap(),
            r#"{{"type":"message_end","message":{{"role":"assistant","content":[]}}}}"#
        )
        .unwrap();
        assert!(capture(Harness::Pi, &path).response.is_empty());
    }

    const TOOL_CALL: &str = r#"{"type":"toolCall","id":"t","name":"read","arguments":{}}"#;

    /// One assistant `message_end` line with the given content parts.
    fn message_end(stop: &str, error: Option<&str>, parts: &[String], input: u64) -> String {
        let error = error
            .map(|error| format!("\"errorMessage\":\"{error}\","))
            .unwrap_or_default();
        format!(
            "{{\"type\":\"message_end\",\"message\":{{\"role\":\"assistant\",\"stopReason\":\"{stop}\",{error}\
             \"content\":[{}],\"usage\":{{\"input\":{input},\"output\":1,\"cost\":{{\"total\":0.5}}}}}}}}\n",
            parts.join(",")
        )
    }

    fn text(text: &str) -> String {
        format!("{{\"type\":\"text\",\"text\":\"{text}\"}}")
    }

    fn append(path: &Path, line: &str) {
        use std::io::Write;
        write!(
            std::fs::OpenOptions::new().append(true).open(path).unwrap(),
            "{line}"
        )
        .unwrap();
    }

    #[test]
    fn a_message_with_tool_calls_is_final_only_for_a_completed_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            message_end("error", Some("old"), &[text("earlier")], 1)
                + &message_end("toolUse", None, &[text("Let me look"), TOOL_CALL.into()], 2),
        )
        .unwrap();

        let ended = capture(Harness::Pi, &path);
        assert_eq!(ended.response, "", "an earlier message never leaks");
        assert_eq!(ended.final_response(false), "", "cut short: a preamble");
        assert_eq!(
            ended.final_response(true),
            "Let me look",
            "a terminating tool call"
        );
        assert_eq!(ended.stop_reason.as_deref(), Some("toolUse"));
        assert_eq!(
            ended.error_message, None,
            "a later message clears the error"
        );
        assert_eq!(ended.turns, 2);
        assert_eq!(ended.tokens_input, 3);
        assert_eq!(ended.tokens_output, 2);
        assert_eq!(ended.cost_usd, 1.0);

        append(&path, &message_end("stop", None, &[text("final")], 4));
        let finished = capture(Harness::Pi, &path);
        assert_eq!(finished.response, "final");
        assert_eq!(finished.final_response(false), "final");
        assert_eq!(finished.final_response(true), "final");
        assert_eq!(finished.stop_reason.as_deref(), Some("stop"));
        assert_eq!(finished.turns, 3);
        assert_eq!(finished.tokens_input, 7);
    }

    #[test]
    fn tool_call_content_marks_a_message_non_final_whatever_its_stop_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            message_end("stop", None, &[text("earlier")], 1)
                + &message_end("length", None, &[text("Truncated"), TOOL_CALL.into()], 1),
        )
        .unwrap();

        let ended = capture(Harness::Pi, &path);
        assert_eq!(ended.response, "");
        assert_eq!(ended.final_response(false), "");
        assert_eq!(ended.final_response(true), "Truncated");
        assert_eq!(ended.stop_reason.as_deref(), Some("length"));
    }

    fn agent() -> Agent {
        Agent {
            name: "explore".into(),
            description: None,
            harness: Harness::Pi,
            models: None,
            model_alias: None,
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
            Harness::Pi,
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
        let args = argv(Harness::Pi, "run-1", &agent, "hello", None);
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
        let args = argv(Harness::Pi, "run-1", &agent, "hello", None);
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

        let capture = capture(Harness::Pi, &path);
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

        let capture = capture(Harness::Pi, &path);
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
            locate(Harness::Pi, &overlay(&format!("{empty}:{first}")), &cwd).unwrap(),
            dir.path().join("first/pi")
        );
        assert_eq!(
            locate(Harness::Pi, &overlay(&format!("{empty}:rel")), &cwd).unwrap(),
            cwd.join("rel/pi")
        );
        assert_eq!(
            locate(Harness::Pi, &overlay(&format!("{empty}::{first}")), &cwd).unwrap(),
            cwd.join("pi")
        );
        let missing = format!(
            "{:#}",
            locate(Harness::Pi, &overlay(&empty), &cwd).unwrap_err()
        );
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
        let found = locate(Harness::Pi, &env, &cwd).unwrap();
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
        let capture = capture(Harness::Pi, Path::new("/nonexistent/events.jsonl"));
        assert_eq!(capture.turns, 0);
        assert!(capture.response.is_empty());
    }

    // ---- claude-code ----

    fn claude_agent() -> Agent {
        Agent {
            name: "cc".into(),
            description: None,
            harness: Harness::ClaudeCode,
            models: None,
            model_alias: None,
            model: Some("claude-haiku-5-5".into()),
            system_prompt: "You review.".into(),
            tools: Some(vec!["Read".into(), "Bash".into()]),
            thinking: Some("high".into()),
            source: Source::User,
            field_sources: Default::default(),
        }
    }

    #[test]
    fn claude_argv_translates_the_pi_shaped_fields() {
        let args = argv(
            Harness::ClaudeCode,
            "run-1",
            &claude_agent(),
            "-find the bug",
            Some(Path::new("/tmp/sp.md")),
        );
        assert_eq!(
            args,
            vec![
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-mode",
                "dontAsk",
                "--model",
                "claude-haiku-5-5",
                "--effort",
                "high",
                "--tools",
                "Read,Bash",
                "--allowedTools",
                "Read,Bash",
                "--append-system-prompt-file",
                "/tmp/sp.md",
                "--",
                "-find the bug",
            ]
        );
    }

    #[test]
    fn claude_argv_maps_tool_and_thinking_edges() {
        let mut agent = claude_agent();
        agent.tools = Some(Vec::new());
        agent.thinking = Some("off".into());
        assert_eq!(
            &argv(Harness::ClaudeCode, "r", &agent, "hi", None)[6..],
            // "off" has no effort counterpart: dropped, not rejected.
            ["--model", "claude-haiku-5-5", "--tools", "", "--", "hi"]
        );
        agent.thinking = None;
        agent.tools = None;
        agent.model = None;
        assert_eq!(
            &argv(Harness::ClaudeCode, "r", &agent, "hi", None)[6..],
            ["--allowedTools", "Read,Edit,Write,Bash", "--", "hi"]
        );
    }

    fn claude_capture(lines: &[&str]) -> Capture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        capture(Harness::ClaudeCode, &path)
    }

    /// A recorded claude-code 2.1.306 run (trimmed to the fields capture
    /// reads): the main agent starts a background sub-agent, ends a first
    /// segment, and is resumed when the sub-agent finishes. Each segment ends
    /// in its own result event.
    const CLAUDE_BACKGROUND_SUBAGENT: &[&str] = &[
        r#"{"type":"system","subtype":"init","session_id":"a5fb","model":"claude-haiku-5-5"}"#,
        r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","model":"claude-haiku-5-5","content":[{"type":"tool_use","id":"toolu_main","name":"Agent","input":{"subagent_type":"general-purpose"}}]}}"#,
        r#"{"type":"user","parent_tool_use_id":null,"message":{"content":[{"type":"tool_result","tool_use_id":"toolu_main"}]}}"#,
        r#"{"type":"assistant","parent_tool_use_id":"toolu_main","message":{"id":"s1","model":"claude-haiku-5-5","content":[{"type":"tool_use","id":"toolu_sub","name":"Bash","input":{"command":"cat probe.txt"}}]}}"#,
        r#"{"type":"user","parent_tool_use_id":"toolu_main","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_sub"}]}}"#,
        r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m2","model":"claude-haiku-5-5","content":[{"type":"thinking","thinking":""}]}}"#,
        r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m2","model":"claude-haiku-5-5","content":[{"type":"text","text":"The sub-agent is still running."}]}}"#,
        r#"{"type":"assistant","parent_tool_use_id":"toolu_main","message":{"id":"s2","model":"claude-haiku-5-5","content":[{"type":"text","text":"SUBAGENT-SAYS hello"}]}}"#,
        r#"{"type":"system","subtype":"init","session_id":"a5fb","model":"claude-haiku-5-5"}"#,
        r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m3","model":"claude-haiku-5-5","content":[{"type":"text","text":"It said: SUBAGENT-SAYS hello"}]}}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"result":"The sub-agent is still running.","stop_reason":"end_turn","num_turns":2,"total_cost_usd":0.004930325,"result_index":0,"modelUsage":{"claude-haiku-5-5":{"inputTokens":12,"outputTokens":547,"cacheReadInputTokens":71400,"cacheCreationInputTokens":25467}}}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"result":"It said: SUBAGENT-SAYS hello","stop_reason":"end_turn","num_turns":1,"total_cost_usd":0.004930325,"result_index":1,"modelUsage":{"claude-haiku-5-5":{"inputTokens":12,"outputTokens":547,"cacheReadInputTokens":71400,"cacheCreationInputTokens":25467}}}"#,
    ];

    #[test]
    fn the_last_claude_result_event_decides_the_run() {
        let capture = claude_capture(CLAUDE_BACKGROUND_SUBAGENT);
        assert_eq!(capture.session_id.as_deref(), Some("a5fb"));
        assert_eq!(capture.model.as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(capture.final_response(true), "It said: SUBAGENT-SAYS hello");
        assert!(!capture.is_error);
        assert_eq!(capture.cost_usd, 0.004930325, "cumulative, not summed");
        // modelUsage is cumulative, sub-agents included.
        assert_eq!(capture.tokens_input, 12);
        assert_eq!(capture.tokens_output, 547);
        assert_eq!(capture.cache_read, 71400);
        assert_eq!(capture.cache_write, 25467);
        let tool = capture.last_tool.unwrap();
        assert_eq!(tool.name, "Agent", "the sub-agent's Bash call is not ours");
        assert!(!tool.pending);
    }

    #[test]
    fn a_claude_run_without_result_falls_back_to_the_main_agents_text() {
        // Killed after the sub-agent spoke but before any result event.
        let capture = claude_capture(&CLAUDE_BACKGROUND_SUBAGENT[..8]);
        assert_eq!(
            capture.final_response(false),
            "The sub-agent is still running."
        );
        assert_eq!(capture.tokens_output, 0);
    }

    #[test]
    fn a_claude_result_event_makes_an_exit_zero_run_failed() {
        let capture = claude_capture(&[
            r#"{"type":"system","subtype":"init","session_id":"u-2","model":"claude-haiku-5-5"}"#,
            r#"{"type":"assistant","message":{"model":"<synthetic>","role":"assistant","stop_reason":"stop_sequence","content":[{"type":"text","text":"Failed to authenticate: OAuth session expired"}],"usage":{}},"is_api_error_message":true}"#,
            r#"{"type":"result","subtype":"success","is_error":true,"terminal_reason":"api_error","result":"Failed to authenticate: OAuth session expired","total_cost_usd":0,"num_turns":1}"#,
        ]);
        assert!(
            capture.is_error,
            "the result event decides, not exit status"
        );
        assert!(capture.response.is_empty());
        assert_eq!(
            capture.error_message.as_deref(),
            Some("Failed to authenticate: OAuth session expired")
        );
        assert_eq!(capture.model.as_deref(), Some("claude-haiku-5-5"));
    }

    #[test]
    fn a_successful_claude_result_clears_an_earlier_error() {
        let capture = claude_capture(&[
            r#"{"type":"assistant","message":{"model":"<synthetic>","content":[{"type":"text","text":"Overloaded"}]},"is_api_error_message":true}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done","num_turns":2}"#,
        ]);
        assert!(!capture.is_error);
        assert_eq!(capture.error_message, None);
        assert_eq!(capture.response, "done");
    }

    #[test]
    fn claude_capture_skips_malformed_and_unknown_lines() {
        let capture = claude_capture(&[
            "not json",
            r#"{"type":"stream_event","event":{}}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","num_turns":1}"#,
        ]);
        assert_eq!(capture.malformed_lines, 1);
        assert_eq!(capture.response, "ok");
    }
}
