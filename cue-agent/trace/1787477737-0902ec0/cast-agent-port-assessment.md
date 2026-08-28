# cast-agent port assessment

Source reviewed: /home/pl/code/palekiwi-labs/cast/worktrees/
feat-cast-agent-mvp (branch `feat/cast-agent-mvp`, crate
`crates/cast-agent`, ~1200 lines src + ~770 lines tests), assessed
2026-08-24 against the cue-agent spec. That worktree may be deleted
at any time — the load-bearing pieces are snapshotted into this
task context (see refs). Do not treat the worktree as a dependency;
consult it only while it exists.

## Verdicts per module

### supervisor.rs (375 lines) — TRANSPLANT near-verbatim

- Cast path: `crates/cast-agent/src/supervisor.rs`
- Local snapshot: `.cue/cue-agent/ref/cast-agent-supervisor.rs`
- Keep as-is: `EndReason` taxonomy (Exited / TimedOut{child_status}
  / Interrupted{trigger, child_status} / SpawnFailed /
  SuperviseFailed), `ProcessGroupGuard` RAII, `send_signal`
  (kill(-pgid), pgid==0 refused, ESRCH ignored), line-writer thread
  with per-line flush, `events_from_disk` fallback, `supervise()`
  control-plane select! over cheap futures, `graceful_teardown`
  with double-signal escalation, bounded reader AND writer joins
  with abort-not-drop semantics.
- Required adaptations (full list with rationale in
  `.cue/cue-agent/ref/supervisor-invariants.md` "Deltas" section):
  - stdin prompt write + EOF -> stdin=/dev/null; prompt via
    prompt.md artifact + `@<path>` positional to pi
  - grace 3s -> 5s
  - `Command::envs` overlay for spec `env`; child cwd set per
    worktree mode
  - signal registration lifted to the orchestrator (one
    registration, cancels all supervisors); per-run teardown logic
    stays intact

### finalize.rs (325 lines) — PARTIAL CARRY

- Cast path: `crates/cast-agent/src/finalize.rs`
- Carry: `classify()` + `exit_info_from_status()` (code-vs-signal
  disposition, fallback signal when unreapable) — 8 unit tests are
  in-file and port directly; atomic receipt write (sibling `.tmp`
  + rename, supervisor.rs pattern in finalize.rs:208-214).
- Drop: the `Verdict` schema (old result.json shape: log_path/
  prompt_path references) and the old exit-code table (0/1/3/4/5
  incl. `crashed` as a separate status). New contract per spec
  section 6: statuses `completed|failed|timed_out|interrupted`
  (`crashed` folds into `failed` with the signal recorded); exit
  0 = all completed, 1 = any failure/timeout/interrupt, 2 = usage.
  Receipts are self-sufficient (embedded response/error, session,
  worktree, env-keys-only).

### run.rs (118 lines) — PATTERN REFERENCE

- `orchestrate()` structure (persist-before-spawn -> supervise ->
  classify -> extract -> write receipt) maps onto the new per-run
  pipeline, but the new pipeline is spec-driven with minted ids and
  cuelib artifacts. Rewrite; use as structural reference only.

### harness/mod.rs + harness/opencode.rs (102 lines) — BUILD FRESH

- The multi-harness `Harness` trait is unnecessary (pi-only MVP;
  future harnesses get a thin adapter later). Replace with a single
  `pi.rs` flag-translation module implementing spec section 4's
  table. The old `extract_result` heuristic is superseded by the
  validated extraction rule in
  `.cue/cue-agent/ref/pi-0.84.2-json-events.md`.

### prompt.rs (63 lines) — BUILD FRESH

- Old precedence (--file > stdin > positional) is replaced by the
  spec's `{file}` interpolation resolved relative to the spec
  file's directory. Unrelated problem; nothing to carry.

### rundir.rs (53 lines) — BUILD FRESH

- TMPDIR-based run dirs are replaced by cuelib tmp artifacts at
  `.cue/<context>/tmp/<timestamp>-<run-id>/`. The timestamp-first
  sortable naming idea survives inside cuelib's convention.

### main.rs (155 lines) — BUILD FRESH

- New CLI surface: `run` subcommand, orchestrator flags only
  (`--task`/`--concurrency`/`--timeout`), receipts array on stdout,
  exit 0/1/2. Old flags (--harness/--file/--timeout/--run-dir/
  --agent + positional prompt) do not survive.

## Test assets to port

- `tests/supervisor_test.rs` (243 lines) and `tests/interrupt_test.rs`
  (241 lines): the scripted fake-binary PATH-shim pattern — write a
  tiny script named like the harness binary, prepend its dir to
  PATH, script behaviors (sleep, self-kill by signal, spawn a
  pipe-holding grandchild, emit JSONL). Rebuild as fake-pi shims
  that replay the validated probe JSONL (fixture commands at the
  bottom of pi-0.84.2-json-events.md).
- `tests/orchestrate_test.rs` (144 lines): injected exe/args
  end-to-end pattern over `orchestrate()` — keep this seam so tests
  never depend on real pi.
- `tests/prompt_test.rs`, `tests/rundir_test.rs`: superseded
  (prompt precedence and run-dir layout are new designs).

## Not portable (spec-divergent surface)

Multi-harness abstraction, stdin prompt protocol, TMPDIR run dirs,
old receipt schema, old exit-code table, `--agent` passthrough
(replaced by agents.json presets in the delegate consumer, not in
cue-agent itself).
