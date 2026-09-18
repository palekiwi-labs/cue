//! One batch, end to end: admission, persist-before-spawn, supervision,
//! capture, receipts and traces.

use crate::cli::RunArgs;
use crate::engine::{self, Disposition, RunPlan};
use crate::harness;
use crate::ids;
use crate::manifest::{self, Agent};
use crate::receipt::{BatchReceipt, Outcome, RunReceipt};
use crate::state;
use crate::trace;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The per-call concurrency cap. Not shared across a session: overlapping calls
/// may run more than this in total, which is accepted for this phase. A batch
/// over the cap fails before anything is spawned rather than queuing, because
/// there is no queue.
pub const MAX_BATCH: usize = 4;

/// How much of a failed run's stderr travels in the receipt.
const STDERR_EXCERPT_BYTES: usize = 2000;

pub struct BatchOutcome {
    pub receipt: BatchReceipt,
    pub all_completed: bool,
}

/// One requested run, before its agent is resolved.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    agent: String,
    prompt: String,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchFile {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    context: Option<String>,
    runs: Vec<Request>,
}

pub fn execute(args: &RunArgs) -> Result<BatchOutcome> {
    let cwd = match &args.cwd {
        Some(path) => path
            .canonicalize()
            .with_context(|| format!("--cwd: could not resolve directory {}", path.display()))?,
        None => std::env::current_dir()?,
    };

    let mut label = args.label.clone();
    let mut context = args.context.clone();
    let requests = collect_requests(args, &mut label, &mut context)?;

    // Admission: validate the whole batch before spawning any of it, so a
    // rejected request never leaves half a batch running.
    if requests.is_empty() {
        bail!("No runs requested: name at least one agent, or pass --batch");
    }
    if requests.len() > MAX_BATCH {
        bail!(
            "Batch of {} exceeds the per-call cap of {MAX_BATCH}. \
             Split the request; cue-agent does not queue the excess.",
            requests.len()
        );
    }

    for request in &requests {
        if request.prompt.trim().is_empty() {
            bail!("The prompt for '{}' is empty", request.agent);
        }
        if request.prompt.starts_with(['-', '@']) {
            bail!(
                "The prompt for '{}' starts with '-' or '@', which Pi interprets as an option or file; prepend ordinary instruction text",
                request.agent
            );
        }
    }

    let manifest = manifest::load(&cwd)?;
    let agents: Vec<&Agent> = requests
        .iter()
        .map(|request| manifest.resolve(&request.agent))
        .collect::<Result<_>>()?;

    let context = context
        .or_else(|| std::env::var("CUE_CONTEXT").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let harness_path = harness::resolve(args.harness.clone())?;
    let harness_name = harness_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| harness_path.to_string_lossy().into_owned());
    let harness_version = harness::version(&harness_path, &cwd);

    let now = SystemTime::now();
    let now_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let batch_id = ids::batch_id(now);
    let agent_root = state::agent_root()?;
    state::private_dir(&agent_root)?;
    let batch_path = state::batch_dir(&agent_root, now_secs, &batch_id);
    state::private_dir(&batch_path)
        .with_context(|| format!("Could not create {}", batch_path.display()))?;

    let store_root = cuelib::store::root(None).ok();
    let repo_scope = cuelib::store::repository_scope(&cwd).ok();
    let commit = cuelib::git::get_short_head_hash(&cwd).ok();

    // Persist before spawn: prompt, system prompt and request manifest exist on
    // disk before the harness starts, so an instant crash loses no record.
    let mut prepared = Vec::with_capacity(requests.len());
    for (index, (request, agent)) in requests.iter().zip(agents.iter()).enumerate() {
        let number = index + 1;
        let run_id = ids::run_id(&batch_id, &agent.name, number);
        // The id names a harness session as well as a directory, so it is
        // checked against the harness's own rule rather than assumed valid.
        if !ids::is_valid(&run_id) {
            bail!("Generated run id '{run_id}' is not a valid harness session id");
        }
        let run_path = batch_path.join(format!("{}-{number}", ids::sanitize(&agent.name)));
        state::private_dir(&run_path)
            .with_context(|| format!("Could not create {}", run_path.display()))?;

        let prompt_path = run_path.join("prompt.md");
        state::write_private(&prompt_path, &request.prompt)?;
        let system_prompt_path = run_path.join("system-prompt.md");
        state::write_private(&system_prompt_path, &agent.system_prompt)?;
        let system_prompt_arg =
            (!agent.system_prompt.trim().is_empty()).then(|| system_prompt_path.clone());

        let argv = harness::argv(
            &run_id,
            agent,
            &request.prompt,
            system_prompt_arg.as_deref(),
        );
        let timeout_secs = request
            .timeout_secs
            .or(args.timeout)
            .or(agent.timeout_secs)
            .filter(|secs| *secs > 0);
        let deadline = timeout_secs.map(Duration::from_secs);

        let manifest_json = serde_json::json!({
            "run_id": run_id,
            "batch_id": batch_id,
            "agent": agent.name,
            "model": agent.model,
            "harness": harness_name,
            "harness_version": harness_version,
            "harness_path": harness_path,
            "argv": argv,
            "cwd": cwd,
            "context": context,
            "repo": repo_scope,
            "commit": commit,
            "store_root": store_root,
            "timeout_secs": timeout_secs,
            "created_at": now_secs,
        });
        state::write_private(
            run_path.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest_json)?,
        )?;

        prepared.push(Prepared {
            plan: RunPlan {
                run_id,
                program: harness_path.clone(),
                argv,
                cwd: cwd.clone(),
                context: context.clone(),
                events_path: run_path.join("events.jsonl"),
                stderr_path: run_path.join("stderr.log"),
                deadline,
            },
            agent: (*agent).clone(),
            run_path,
        });
    }

    // Registration belongs to the host, not to supervision: one handler covers
    // the whole batch.
    engine::install_signal_handlers();
    let plans: Vec<RunPlan> = prepared.iter().map(|run| run.plan.clone()).collect();
    let outcomes = engine::supervise(&plans);

    let mut runs = Vec::with_capacity(prepared.len());
    for (prepared, outcome) in prepared.into_iter().zip(outcomes) {
        let capture = harness::capture(&prepared.plan.events_path);
        let (result, exit_code, signal, error) = classify(&outcome.disposition, &capture);

        let stderr_excerpt = read_stderr_excerpt(&prepared.plan.stderr_path)
            .filter(|excerpt| !excerpt.trim().is_empty());

        let mut run = RunReceipt {
            run_id: prepared.plan.run_id.clone(),
            batch_id: batch_id.clone(),
            agent: prepared.agent.name.clone(),
            model: prepared.agent.model.clone().or(capture.model.clone()),
            outcome: result,
            exit_code,
            signal,
            duration_ms: outcome.duration.as_millis(),
            turns: capture.turns,
            tokens_input: capture.tokens_input,
            tokens_output: capture.tokens_output,
            cost_usd: capture.cost_usd,
            response: capture.response.clone(),
            error,
            stderr_excerpt,
            events_malformed: capture.malformed_lines,
            events_oversized: capture.oversized_lines,
            events_truncated: capture.truncated,
            run_path: prepared.run_path.clone(),
            trace: None,
            trace_error: None,
            persistence_errors: Vec::new(),
        };

        if let Some(context) = &context {
            match write_trace(
                &run,
                &prepared,
                &cwd,
                context,
                label.as_deref(),
                &harness_name,
                harness_version.as_deref(),
            ) {
                Ok(address) => run.trace = address,
                Err(err) => run.trace_error = Some(format!("{err:#}")),
            }
        }

        if let Err(err) = state::append_index(
            &agent_root,
            &serde_json::json!({
                "timestamp": now_secs,
                "run_id": run.run_id,
                "batch_id": run.batch_id,
                "agent": run.agent,
                "model": run.model,
                "context": context,
                "repo": repo_scope,
                "commit": commit,
                "outcome": run.outcome.as_str(),
                "exit_code": run.exit_code,
                "duration_ms": run.duration_ms,
                "cost_usd": run.cost_usd,
                "path": run.run_path,
            }),
        ) {
            run.persistence_errors.push(format!("index.jsonl: {err:#}"));
        }
        let persist = || -> Result<()> {
            state::write_private(
                prepared.run_path.join("receipt.json"),
                serde_json::to_vec_pretty(&run)?,
            )?;
            Ok(())
        };
        if let Err(err) = persist() {
            run.persistence_errors
                .push(format!("receipt.json: {err:#}"));
        }

        runs.push(run);
    }

    let all_completed = runs
        .iter()
        .all(|run| run.outcome == Outcome::Completed && run.persistence_errors.is_empty());
    Ok(BatchOutcome {
        receipt: BatchReceipt {
            batch_id,
            batch_path,
            cap: MAX_BATCH,
            context,
            harness: harness_name,
            harness_version,
            runs,
        },
        all_completed,
    })
}

struct Prepared {
    plan: RunPlan,
    agent: Agent,
    run_path: PathBuf,
}

/// Map a disposition and what the stream said into a receipt outcome.
///
/// A harness that exits 0 while reporting an error stop reason is a failure:
/// the exit status alone would call it a success and hand the caller an empty
/// response.
fn classify(
    disposition: &Disposition,
    capture: &harness::Capture,
) -> (Outcome, Option<i32>, Option<i32>, Option<String>) {
    match disposition {
        Disposition::SpawnFailed { message } | Disposition::WaitFailed { message } => {
            (Outcome::Failed, None, None, Some(message.clone()))
        }
        Disposition::TimedOut { code, signal } => (
            Outcome::Timeout,
            *code,
            *signal,
            Some("the run passed its deadline and was terminated".to_string()),
        ),
        Disposition::Aborted {
            trigger,
            code,
            signal,
        } => (
            Outcome::Aborted,
            *code,
            *signal,
            Some(format!("the batch was interrupted by signal {trigger}")),
        ),
        Disposition::Exited { code, signal } => {
            let failed_stop = matches!(
                capture.stop_reason.as_deref(),
                Some("error") | Some("aborted")
            );
            if *code == Some(0) && !failed_stop {
                (Outcome::Completed, *code, *signal, None)
            } else {
                let error = capture.error_message.clone().or_else(|| match signal {
                    Some(signal) => Some(format!("the harness was killed by signal {signal}")),
                    None => code.map(|code| format!("the harness exited with status {code}")),
                });
                (Outcome::Failed, *code, *signal, error)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_trace(
    run: &RunReceipt,
    prepared: &Prepared,
    cwd: &Path,
    context: &str,
    label: Option<&str>,
    harness_name: &str,
    harness_version: Option<&str>,
) -> Result<Option<String>> {
    // The body is the final message verbatim and nothing else, so a run that
    // produced no message gets no trace. Plane 2 preserves the run either way,
    // and the trace can be materialised later from it.
    if run.response.is_empty() {
        return Ok(None);
    }
    let body_path = prepared.run_path.join("response.body");
    state::write_private(&body_path, &run.response)?;
    let name = trace::artifact_name(
        label.unwrap_or("run"),
        &prepared.agent.name,
        &ids::short(&run.run_id),
    );
    let fields = trace::frontmatter(run, harness_name, harness_version, label);
    let address = trace::write(cwd, context, &name, &body_path, &fields);
    // The body file is a transport detail for `cue add`, not a third copy of
    // the response, so it does not survive the write.
    let _ = std::fs::remove_file(&body_path);
    address.map(Some)
}

fn read_stderr_excerpt(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(STDERR_EXCERPT_BYTES as u64);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity(STDERR_EXCERPT_BYTES);
    file.take(STDERR_EXCERPT_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    // Do not manufacture a replacement character when the tail starts inside
    // an otherwise valid UTF-8 codepoint. Invalid bytes elsewhere remain lossy.
    let skip = if start > 0 {
        bytes
            .iter()
            .take_while(|byte| **byte & 0xc0 == 0x80)
            .count()
    } else {
        0
    };
    Some(String::from_utf8_lossy(&bytes[skip..]).into_owned())
}

/// Turn the command line into the runs it asks for.
fn collect_requests(
    args: &RunArgs,
    label: &mut Option<String>,
    context: &mut Option<String>,
) -> Result<Vec<Request>> {
    if let Some(source) = &args.batch {
        if !args.agents.is_empty() {
            bail!("--batch carries its own agents; do not also name agents positionally");
        }
        let raw = read_source(source)?;
        let file = parse_batch(&raw)?;
        if label.is_none() {
            *label = file.label;
        }
        if context.is_none() {
            *context = file.context;
        }
        return Ok(file.runs);
    }

    let prompt = match (&args.prompt, &args.prompt_file) {
        (Some(text), _) => text.clone(),
        (None, Some(source)) => read_source(source)?,
        (None, None) => bail!("A prompt is required: pass --prompt, --prompt-file or --batch"),
    };
    if prompt.trim().is_empty() {
        bail!("The prompt is empty");
    }

    Ok(args
        .agents
        .iter()
        .map(|agent| Request {
            agent: agent.clone(),
            prompt: prompt.clone(),
            timeout_secs: None,
        })
        .collect())
}

/// A batch is an object with `runs`, or the bare array of runs.
fn parse_batch(raw: &str) -> Result<BatchFile> {
    let value: serde_json::Value =
        serde_json::from_str(raw).context("The batch is not valid JSON")?;
    if value.is_array() {
        let runs: Vec<Request> =
            serde_json::from_value(value).context("Invalid run in the batch")?;
        return Ok(BatchFile {
            label: None,
            context: None,
            runs,
        });
    }
    serde_json::from_value(value).context("Invalid batch")
}

fn read_source(source: &str) -> Result<String> {
    if source == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("Could not read standard input")?;
        return Ok(buffer);
    }
    std::fs::read_to_string(source).with_context(|| format!("Could not read {source}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn stderr_excerpt_reads_only_a_bounded_tail_of_a_sparse_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stderr.log");
        let mut file = std::fs::File::create(&path).unwrap();
        file.seek(SeekFrom::Start(8 * 1024 * 1024 * 1024)).unwrap();
        file.write_all("é".as_bytes()).unwrap();
        file.write_all(&vec![b'x'; STDERR_EXCERPT_BYTES - 1])
            .unwrap();
        assert_eq!(
            read_stderr_excerpt(&path).unwrap(),
            "x".repeat(STDERR_EXCERPT_BYTES - 1)
        );
    }
}
