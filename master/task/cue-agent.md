---
title: cue-agent crate
status: open
priority: normal
kind: build
branch: feat/cue-agent
refs:
  - .cue/cue-agent/spec/index.md
  - .cue/cue-agent/plan/index.md
  - .cue/cue-agent/ref/pi-0.84.2-api.md
  - .cue/cue-agent/ref/pi-0.84.2-json-events.md
  - .cue/cue-agent/ref/supervisor-invariants.md
  - .cue/cue-agent/ref/cast-agent-supervisor.rs
  - /home/pl/code/palekiwi/palekiwi/.cue/master/task/cue-agent.md
---

# cue-agent — supervised pi launcher & batch engine

## Context

Implement `crates/cue-agent` in this repo: a process-isolated,
supervised launcher and batch engine for headless `pi` agents. It
reads a JSON execution spec (array of agent runs), spawns supervised
`pi -p --mode json` child processes with timeout enforcement and
process-group teardown, and writes self-sufficient receipts to stdout
plus trace artifacts into a cue context via cuelib.

The design was completed upstream in the palekiwi coordination
workspace (design task `design-cue-agent`), which also hosts the
umbrella coordination card for this work:
`/home/pl/code/palekiwi/palekiwi/.cue/master/task/cue-agent.md`.
Everything the
implementing agent needs is snapshotted into this task's context —
it is self-contained; no access to the palekiwi workspace is
required:

- Authoritative spec: `.cue/cue-agent/spec/index.md`
- pi 0.84.2 flag/API surface: `.cue/cue-agent/ref/pi-0.84.2-api.md`
- pi 0.84.2 JSON event schema (live-probe validated, incl. the
  final-assistant-message extraction rule):
  `.cue/cue-agent/ref/pi-0.84.2-json-events.md`
- Port kit from the abandoned cast-agent prototype:
  `.cue/cue-agent/ref/cast-agent-supervisor.rs` (verbatim source,
  starting point for the supervisor) and
  `.cue/cue-agent/ref/supervisor-invariants.md` (load-bearing rules)
- Port assessment (transplant / adapt / build-fresh per module):
  `.cue/cue-agent/trace/1787477737-0902ec0/cast-agent-port-assessment.md`
- Seeded master plan: `.cue/cue-agent/plan/index.md`

## Scope

MVP exactly as specced: pi-only harness; orchestrator CLI flags only
(`--task`, `--concurrency`, `--timeout`); JSON spec array input
(positional / `--spec-file` / stdin); receipts array on stdout; exit
codes 0/1/2; cuelib tmp artifacts; session-id minting with
`--session-dir` pinning; worktree modes (cwd/ephemeral/named); env
overlay with redaction. Explicitly out of scope: background
execution (reject `background: true`), `--stream`, harnesses other
than pi, the `delegate` opencode tool (separate repo:
cue-plugins), the cue-review consumer.

## Acceptance criteria

- All phase checkboxes in `.cue/cue-agent/plan/index.md` complete.
- Unit tests + scripted fake-pi PATH-shim integration tests green
  (`cargo test`).
- Real-pi smoke test (pong probe + tool-use probe) run against a
  scratch cue context produces correct receipts and artifacts.
- Clippy and fmt clean; crate follows repo conventions (AGENTS.md,
  `.cue/master/spec/index.md`).

## Working rules

- Work on branch `feat/cue-agent` (worktree
  `worktrees/feat-cue-agent`, base `master`).
- TDD: fake-pi shims first, real-pi smoke last.
- Log milestones and decisions to this task context as you go.
