//! One batch, end to end: admission, persist-before-spawn, supervision,
//! capture, receipts and traces.

use crate::cli::RunArgs;
use crate::engine::{self, Disposition, RunOutcome, RunPlan};
use crate::harness;
use crate::ids;
use crate::manifest::{self, Agent};
use crate::receipt::{BatchReceipt, Outcome, RunReceipt};
use crate::run_spec::{self, Bases, MAX_TASKS, ResolvedBatch};
use crate::state;
use crate::trace;
use crate::worktree;
use anyhow::{Context, Result, anyhow, bail};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How much of a failed run's stderr travels in the receipt.
const STDERR_EXCERPT_BYTES: usize = 2000;

pub struct BatchOutcome {
    pub receipt: BatchReceipt,
    pub all_completed: bool,
}

/// An admitted request and the settings it runs under. Building one touches
/// nothing but the specification, its referenced files, the manifest and the
/// store's context records.
struct Admitted {
    request: ResolvedBatch,
    timeout_secs: Option<u64>,
    /// Each task's configured worktree root, resolved against its target.
    worktree_roots: Vec<Option<PathBuf>>,
}

pub fn execute(args: &RunArgs) -> Result<BatchOutcome> {
    let admitted = admit(args)?;
    run(admitted)
}

/// Admission: resolve and validate the whole request before any run state is
/// created, any harness is probed or launched, or any trace is written, so a
/// rejected request leaves nothing behind.
fn admit(args: &RunArgs) -> Result<Admitted> {
    let invocation =
        std::env::current_dir().context("Could not determine the invocation directory")?;
    let (raw, references) = read_specification(args, &invocation)?;
    // Agent names come from the manifest discovered from the invocation
    // directory, whatever cwd a task later runs in.
    let manifest = manifest::load(&invocation)?;
    let store = cuelib::store::root(None)?;
    let request = run_spec::resolve(
        &raw,
        Bases {
            references: &references,
            invocation: &invocation,
            store: &store,
        },
        &manifest,
    )?;
    // The CLI overrides the manifest; zero means no deadline.
    let timeout_secs = Some(args.timeout.unwrap_or(manifest.timeout)).filter(|secs| *secs > 0);
    let worktree_roots = request
        .tasks
        .iter()
        .map(|task| manifest.worktree_root(&task.cwd))
        .collect();
    Ok(Admitted {
        request,
        timeout_secs,
        worktree_roots,
    })
}

/// Read the specification from exactly one source. A `--spec` path is joined
/// to the invocation directory but not canonicalized, so its references
/// resolve beside the path the caller named even when it is a symlink.
fn read_specification(args: &RunArgs, invocation: &Path) -> Result<(String, PathBuf)> {
    if let Some(path) = &args.spec {
        let path = invocation.join(path);
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read the run specification {}", path.display()))?;
        let base = path.parent().unwrap_or(invocation).to_path_buf();
        return Ok((raw, base));
    }
    let raw = match args.input.as_deref() {
        Some("-") => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .context("Could not read the run specification from standard input")?;
            buffer
        }
        Some(json) => json.to_string(),
        None => bail!("A run specification is required: pass JSON, '-' or --spec PATH"),
    };
    Ok((raw, invocation.to_path_buf()))
}

/// One task, with its run directory written and its launch decided.
struct Prepared {
    plan: RunPlan,
    /// One-based position in the original tasks array.
    position: usize,
    agent: Agent,
    run_path: PathBuf,
    context: Option<String>,
    label: Option<String>,
    harness_version: Option<String>,
    /// Set when the harness will not be launched for this task, decided
    /// before supervision: a failed preparation or lookup, or an interrupt
    /// that arrived first. The other tasks still run.
    not_launched: Option<Disposition>,
    /// The worktree created for this task, owned until finalization.
    worktree: Option<worktree::Owned>,
    /// Resources a failed preparation could not remove.
    cleanup_errors: Vec<String>,
    /// Local records that could not be written for a task already failing
    /// for another reason.
    record_errors: Vec<String>,
    /// The request record as written before launch, completed with the
    /// checkout's final revision before the checkout may be removed.
    manifest: serde_json::Value,
}

/// Probe answers shared between tasks identical in executable, cwd and
/// environment. Held in memory only; no environment value is persisted.
type Probes = Vec<((PathBuf, PathBuf, harness::EnvOverlay), Option<String>)>;

/// Where a batch keeps its local records.
struct Records<'a> {
    batch_id: &'a str,
    batch_path: &'a Path,
    /// Why the batch directory could not be created, if it could not.
    unavailable: Option<&'a str>,
    now_secs: i64,
    store_root: Option<&'a Path>,
    timeout_secs: Option<u64>,
    deadline: Option<Duration>,
}

/// Everything after admission. Preparation and launch failures are isolated
/// per task, including a local run record that cannot be written: that task
/// fails before launch, anything it created is cleaned up at finalization,
/// and the others continue.
fn run(admitted: Admitted) -> Result<BatchOutcome> {
    let Admitted {
        request,
        timeout_secs,
        worktree_roots,
    } = admitted;

    // Registration belongs to the host, not to supervision. It comes before
    // any resource is created, so an interrupt during preparation is observed
    // and cleaned up after rather than ending the process.
    engine::install_signal_handlers();

    let now = SystemTime::now();
    let now_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let batch_id = ids::batch_id(now);
    // Without a batch directory no task has a record to run from; every task
    // then fails preparation and the batch still reports its results.
    let agent_root = state::agent_root();
    let batch_path = agent_root
        .as_ref()
        .map(|root| state::batch_dir(root, now_secs, &batch_id))
        .unwrap_or_default();
    let unavailable = match &agent_root {
        Err(err) => Some(format!("{err:#}")),
        Ok(root) => state::private_dir(root)
            .and_then(|()| state::private_dir(&batch_path))
            .err()
            .map(|err| format!("could not create {}: {err}", batch_path.display())),
    };

    let store_root = cuelib::store::root(None).ok();
    let records = Records {
        batch_id: &batch_id,
        batch_path: &batch_path,
        unavailable: unavailable.as_deref(),
        now_secs,
        store_root: store_root.as_deref(),
        timeout_secs,
        deadline: timeout_secs.map(Duration::from_secs),
    };

    let mut probes: Probes = Vec::new();
    let mut prepared = Vec::with_capacity(request.tasks.len());
    prepare_all(
        &request,
        worktree_roots,
        &records,
        &mut probes,
        &mut prepared,
    );

    let mut versions = probes.iter().map(|(_, version)| version);
    let harness_version = match versions.next() {
        Some(first) if versions.all(|version| version == first) => first.clone(),
        _ => None,
    };

    // An interrupt that arrived during preparation launches nothing more.
    if let Some(signal) = engine::abort_requested() {
        for run in prepared.iter_mut().filter(|run| run.not_launched.is_none()) {
            run.not_launched = Some(aborted_before_launch(signal));
        }
    }
    let plans: Vec<RunPlan> = prepared
        .iter()
        .filter(|run| run.not_launched.is_none())
        .map(|run| run.plan.clone())
        .collect();
    let mut outcomes = engine::supervise(&plans).into_iter();

    let mut runs = Vec::with_capacity(prepared.len());
    for prepared in prepared {
        let outcome = match &prepared.not_launched {
            Some(disposition) => RunOutcome {
                disposition: disposition.clone(),
                duration: Duration::ZERO,
            },
            None => outcomes
                .next()
                .expect("supervision returns one outcome per launched plan"),
        };
        let launched = prepared.not_launched.is_none()
            && !matches!(outcome.disposition, Disposition::SpawnFailed { .. });
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
            persistence_errors: prepared.record_errors.clone(),
            cleanup_errors: prepared.cleanup_errors.clone(),
            worktree: None,
        };

        // Capture snapshots the checkout's provenance, so it happens before
        // any removal.
        if let Some(context) = &prepared.context {
            match write_trace(&run, &prepared, context, request.label.as_deref()) {
                Ok(address) => run.trace = address,
                Err(err) => run.trace_error = Some(format!("{err:#}")),
            }
        }

        // Cleanup runs whatever capture did. A checkout whose harness never
        // started holds no work, so it is removed even when persistent.
        let mut worktree_record = None;
        if let Some(owned) = &prepared.worktree {
            let head = if launched { owned.head() } else { None };
            let dispose = owned.ephemeral || !launched;
            // The execution revision goes into the run's manifest before the
            // checkout can go. A failed write is reported, never a reason to
            // skip cleanup.
            if launched {
                let mut manifest = prepared.manifest.clone();
                manifest["worktree"]["head"] = serde_json::json!(head);
                let record = || -> Result<()> {
                    state::write_private(
                        prepared.run_path.join("manifest.json"),
                        serde_json::to_vec_pretty(&manifest)?,
                    )?;
                    Ok(())
                };
                if let Err(err) = record() {
                    run.persistence_errors
                        .push(format!("manifest.json: {err:#}"));
                }
            }
            if dispose {
                run.cleanup_errors.extend(worktree::remove(owned));
            } else {
                run.worktree = owned.retained();
            }
            worktree_record = Some(serde_json::json!({
                "path": owned.path,
                "branch": owned.branch,
                "ephemeral": owned.ephemeral,
                "head": head,
                "retained": !dispose,
            }));
        }

        let indexed = agent_root.as_ref().map_err(|err| anyhow!("{err:#}"));
        if let Err(err) = indexed.and_then(|agent_root| {
            state::append_index(
                agent_root,
                &serde_json::json!({
                    "timestamp": now_secs,
                    "run_id": run.run_id,
                    "batch_id": run.batch_id,
                    "agent": run.agent,
                    "model": run.model,
                    "context": prepared.context,
                    "cwd": prepared.plan.cwd,
                    "worktree": worktree_record,
                    "outcome": run.outcome.as_str(),
                    "exit_code": run.exit_code,
                    "duration_ms": run.duration_ms,
                    "cost_usd": run.cost_usd,
                    "path": run.run_path,
                }),
            )
        }) {
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

    // Execution, storage and cleanup outcomes stay separate in each receipt,
    // but any failed finalization, an explicit capture that was not written
    // included, fails the batch.
    let all_completed = runs.iter().all(|run| {
        run.outcome == Outcome::Completed
            && run.trace_error.is_none()
            && run.persistence_errors.is_empty()
            && run.cleanup_errors.is_empty()
    });
    Ok(BatchOutcome {
        receipt: BatchReceipt {
            batch_id,
            batch_path,
            cap: MAX_TASKS,
            harness: harness::PROGRAM.to_string(),
            harness_version,
            runs,
        },
        all_completed,
    })
}

fn aborted_before_launch(trigger: i32) -> Disposition {
    Disposition::Aborted {
        trigger,
        code: None,
        signal: None,
    }
}

/// Persist before spawn and prepare each task in specification order:
/// prompt and system prompt are on disk before its worktree or harness, and
/// the request manifest records the checkout it will run in. A record that
/// cannot be written fails only its own task, before launch: its worktree,
/// if one was created, stays owned and is removed at finalization.
fn prepare_all(
    request: &ResolvedBatch,
    worktree_roots: Vec<Option<PathBuf>>,
    records: &Records,
    probes: &mut Probes,
    prepared: &mut Vec<Prepared>,
) {
    for (index, (task, root)) in request.tasks.iter().zip(worktree_roots).enumerate() {
        let number = index + 1;
        let agent = task.agent.clone();
        let run_id = ids::run_id(records.batch_id, &agent.name, number);
        let run_path = records
            .batch_path
            .join(format!("{}-{number}", ids::sanitize(&agent.name)));
        let system_prompt_path = run_path.join("system-prompt.md");
        let system_prompt_arg =
            (!agent.system_prompt.is_empty()).then(|| system_prompt_path.clone());
        let argv = harness::argv(&run_id, &agent, &task.prompt, system_prompt_arg.as_deref());
        let label = task.label.clone().or_else(|| request.label.clone());

        let mut not_launched = None;
        // The id names a harness session as well as a directory, so it is
        // checked against the harness's own rule rather than assumed valid.
        if !ids::is_valid(&run_id) {
            not_launched = Some(failed_before_launch(format!(
                "generated run id '{run_id}' is not a valid harness session id"
            )));
        }
        let recorded = not_launched.is_none()
            && match write_request(records, &run_path, &task.prompt, &agent.system_prompt) {
                Ok(()) => true,
                Err(err) => {
                    not_launched = Some(failed_before_launch(format!(
                        "could not write the local run record: {err:#}"
                    )));
                    false
                }
            };

        let mut owned = None;
        let mut cleanup_errors = Vec::new();
        let mut cwd = task.cwd.clone();
        if not_launched.is_some() {
            // Nothing is created for a task that cannot be recorded.
        } else if let Some(signal) = engine::abort_requested() {
            not_launched = Some(aborted_before_launch(signal));
        } else if let Some(requested) = &task.worktree {
            // Finalization removes checkouts in specification order, so one
            // nested in another task's checkout would be deleted with it.
            let others: Vec<PathBuf> = prepared
                .iter()
                .filter_map(|run| run.worktree.as_ref().map(|owned| owned.path.clone()))
                .collect();
            match worktree::prepare(requested, &task.cwd, root.as_deref(), &run_id, &others) {
                Ok(created) => {
                    cwd = created.path.clone();
                    owned = Some(created);
                }
                Err(failure) => {
                    not_launched = Some(failed_before_launch(failure.message));
                    cleanup_errors = failure.cleanup_errors;
                }
            }
        }
        if not_launched.is_none()
            && let Some(signal) = engine::abort_requested()
        {
            not_launched = Some(aborted_before_launch(signal));
        }

        // Each task finds pi on its own effective PATH, from the directory it
        // will run in, and is probed exactly as it will run.
        let mut program = None;
        let mut version = None;
        if not_launched.is_none() {
            match harness::locate(&task.env, &cwd) {
                Ok(found) => {
                    let key = (found.clone(), cwd.clone(), task.env.clone());
                    version = match probes.iter().find(|(probe, _)| *probe == key) {
                        Some((_, version)) => version.clone(),
                        None => {
                            let version = harness::version(&found, &cwd, &task.env);
                            probes.push((key, version.clone()));
                            version
                        }
                    };
                    program = Some(found);
                }
                Err(err) => {
                    not_launched = Some(failed_before_launch(format!("{err:#}")));
                }
            }
        }
        let launch_error = match &not_launched {
            Some(Disposition::SpawnFailed { message }) => Some(message.clone()),
            Some(Disposition::Aborted { trigger, .. }) => Some(format!(
                "the batch was interrupted by signal {trigger} before launch"
            )),
            _ => None,
        };

        // Environment values are deliberately absent: an overlay may carry
        // secrets, and no persistence policy for them exists yet.
        let source = trace::provenance(&cwd);
        let manifest_json = serde_json::json!({
            "run_id": run_id,
            "batch_id": records.batch_id,
            "agent": agent.name,
            "model": agent.model,
            "harness": harness::PROGRAM,
            "harness_version": version,
            "harness_path": program,
            "argv": argv,
            "cwd": cwd,
            "worktree": owned.as_ref().map(worktree::Owned::record),
            "context": task.context,
            "label": label,
            "batch_label": request.label,
            "repo": source.repo_id,
            "commit": source.commit_hash,
            "store_root": records.store_root,
            "timeout_secs": records.timeout_secs,
            "launch_error": launch_error,
            "created_at": records.now_secs,
        });

        // The request record is the last step before launch. A task that
        // cannot have one is not launched; one already failing keeps its own
        // error and reports the missing record alongside it.
        let mut record_errors = Vec::new();
        if recorded {
            let written = serde_json::to_vec_pretty(&manifest_json)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| {
                    state::write_private(run_path.join("manifest.json"), bytes)
                        .map_err(anyhow::Error::from)
                });
            if let Err(err) = written {
                let message =
                    format!("could not write the local run record manifest.json: {err:#}");
                if not_launched.is_none() {
                    not_launched = Some(failed_before_launch(message));
                } else {
                    record_errors.push(format!("manifest.json: {err:#}"));
                }
            }
        }

        prepared.push(Prepared {
            plan: RunPlan {
                run_id,
                program: program.unwrap_or_else(|| PathBuf::from(harness::PROGRAM)),
                argv,
                cwd,
                env: task.env.clone(),
                events_path: run_path.join("events.jsonl"),
                stderr_path: run_path.join("stderr.log"),
                deadline: records.deadline,
            },
            position: number,
            agent,
            run_path,
            context: task.context.clone(),
            label,
            harness_version: version,
            not_launched,
            worktree: owned,
            cleanup_errors,
            record_errors,
            manifest: manifest_json,
        });
    }
}

fn failed_before_launch(message: String) -> Disposition {
    Disposition::SpawnFailed { message }
}

/// Write the task's run directory, prompt and system prompt.
fn write_request(
    records: &Records,
    run_path: &Path,
    prompt: &str,
    system_prompt: &str,
) -> Result<()> {
    if let Some(reason) = records.unavailable {
        bail!("{reason}");
    }
    state::private_dir(run_path)
        .with_context(|| format!("could not create {}", run_path.display()))?;
    for (name, text) in [("prompt.md", prompt), ("system-prompt.md", system_prompt)] {
        let path = run_path.join(name);
        state::write_private(&path, text)
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(())
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

/// Promote the final response into the task's capture context.
fn write_trace(
    run: &RunReceipt,
    prepared: &Prepared,
    context: &str,
    batch_label: Option<&str>,
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
        batch_label,
        &run.batch_id,
        prepared.position,
        &prepared.agent.name,
    );
    // The repository and revision the run executed, read now, while an
    // ephemeral checkout still exists, and stamped explicitly.
    let source = trace::provenance(&prepared.plan.cwd);
    let fields = trace::frontmatter(
        run,
        harness::PROGRAM,
        prepared.harness_version.as_deref(),
        prepared.label.as_deref(),
        batch_label,
        &source,
    );
    let address = trace::write(&prepared.plan.cwd, context, &name, &body_path, &fields);
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
