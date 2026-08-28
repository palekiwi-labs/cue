---
status: open
refs:
- .cue/cue-agent/spec/index.md
- .cue/cue-agent/ref/pi-0.84.2-json-events.md
- .cue/cue-agent/ref/pi-0.84.2-api.md
- .cue/cue-agent/ref/supervisor-invariants.md
parent: .cue/master/task/cue-agent.md
---
---
title: cue-agent implementation master plan
status: open
priority: high
parent: .cue/master/task/cue-agent.md
refs:
  - .cue/cue-agent/spec/index.md
  - .cue/cue-agent/ref/pi-0.84.2-json-events.md
  - .cue/cue-agent/ref/pi-0.84.2-api.md
  - .cue/cue-agent/ref/supervisor-invariants.md
---

# cue-agent master plan

Implementation is phased; cut an executive plan per phase before
starting it. TDD throughout: scripted fake-pi shims on PATH for
unit/integration tests; real-pi smoke only at the end (fixture
commands in pi-0.84.2-json-events.md). Read the spec
(`.cue/cue-agent/spec/index.md`) and supervisor-invariants.md before
phase 1.

## Phase 0 — scaffold & spec parsing

- [x] `crates/cue-agent` skeleton (lib + bin) wired into the
      workspace; `cargo build`, `cargo test`, clippy, fmt clean
- [x] CLI shell: `cue-agent run [FLAGS] [JSON_SPEC]` with the three
      input modes (positional / `--spec-file` / `-` stdin) and
      orchestrator flags `--task`, `--concurrency`, `--timeout`;
      usage errors exit 2 with stderr message, nothing on stdout
- [x] Spec model: full field reference (spec section 3) — serde
      structs, defaults (`harness: pi`, `approve: false`,
      `session: {persist:false}`, `worktree: {mode: cwd}`,
      `background: false`), `{file}` interpolation relative to spec
      dir, validation (unknown fields rejected; duplicate run ids
      rejected; `background: true` rejected; `harness != pi`
      rejected; `prompt` required) — all exit 2 paths unit-tested

## Phase 1 — single supervised run (the core)

- [ ] Supervisor port from
      `.cue/cue-agent/ref/cast-agent-supervisor.rs` with the deltas
      in supervisor-invariants.md; fake-pi shim tests: clean exit,
      nonzero exit, signal death, wall-clock timeout, SIGINT/SIGTERM
      teardown, grandchild pipe-holder (join-timeout + disk fallback)
- [ ] pi flag translation module (spec section 4 table): per-field
      unit tests incl. `--no-approve` default, tools comma-join,
      append-system-prompt multi-occurrence, `--no-session`
- [ ] Prompt delivery: write resolved prompt to the run's
      `prompt.md` artifact (persist-before-spawn), pass `@<path>`
      positional to pi; stdin=/dev/null
- [ ] Run artifacts via cuelib tmp:
      `.cue/<ctx>/tmp/<timestamp>-<run-id>/` with prompt.md,
      system-prompt.md, stream.jsonl (live-flushed), stderr.log
- [ ] Event stream parser per pi-0.84.2-json-events.md: typed
      events, session-header validation vs minted id (tolerate
      EMPTY streams — pre-flight failure case), final-message
      extraction (agent_end rule + message_end fallback), ANSI
      strip for stderr tails
- [ ] Receipt build + atomic write: self-sufficient receipt.json
      (spec section 6 field list); statuses
      `completed|failed|timed_out|interrupted`; response also
      written as response.md

## Phase 2 — batch orchestration

- [ ] Pre-spawn id minting: run ids (default `run-N`, batch-unique)
      and session UUIDv7 where `persist` — before any spawn
- [ ] Concurrency semaphore (`--concurrency`, 0 = unbounded) with
      per-run failure isolation (one run's crash/timeout never
      cancels siblings)
- [ ] Batch wall-clock `--timeout`; orchestrator-level single
      SIGINT/SIGTERM registration cancelling all supervisors (each
      does its own two-phase teardown)
- [ ] stdout contract: valid receipts JSON array whenever the spec
      parses (even if all runs failed); exit codes 0/1/2 mapping
- [ ] `batch-receipt.json` summary artifact (duration, concurrency,
      per-run id/status/duration; references runs, does not embed)

## Phase 3 — sessions & worktrees

- [ ] Session persist path: mint/accept `--session-id`, pin
      `--session-dir` to `.cue/<context>/sessions/`, receipt
      session block `{persist, id, file}`
- [ ] Resume smoke: re-passed session.id runs continue the
      conversation (header echo sanity check)
- [ ] Worktree modes: `cwd` (default) / `ephemeral` / `named` with
      `base` ref; child cwd set accordingly; receipt worktree block
- [ ] Ephemeral worktree lifecycle: created before spawn, removed
      after teardown on ALL paths (success, failure, timeout,
      interrupt); no orphans left in failure cascades

## Phase 4 — integration & hardening

- [ ] Real-pi smoke tests (scratch cue context): pong probe,
      tool-use probe, bad-model failure probe; verify receipts,
      artifacts, and session files land as specced
- [ ] Redaction audit: no env VALUES in receipts/traces; keys only
- [ ] Crate docs/README: usage examples (spec file, single run,
      parallel batch, session resume)
- [ ] Full gate: `cargo test`, clippy, fmt; logs updated; PR
      against master ready

## Notes

- cuelib integration for tmp artifacts: inspect cuelib's API for
  point-in-time artifact creation; if a gap exists (e.g. writing
  into `.cue/<ctx>/tmp/<ts>-<name>/` dirs), surface it early —
  do not silently invent a parallel persistence path.
- The `--task` context resolution should reuse cuelib's logic
  (`.cue/HEAD` default, never mutate).
