---
title: "Task workflow — Phase 2: HEAD-driven context directories"
status: in-progress
priority: high
branch:
  - feat/task-mode
refs:
  - .cue/master/spec/cue/task-mode.md
  - .cue/feat-task-mode/plan/index.md
---

# Task workflow — Phase 2: HEAD-driven context directories

Implement HEAD-driven context scope in the `cue` CLI. Replace git-branch-derived
scope with `.cue/HEAD`-derived scope across all write paths. Introduce `cue switch`,
`cue status`, and a `--task` flag on write commands.

## Source

- `.cue/master/spec/cue/task-mode.md` — full specification
- `.cue/feat-task-mode/plan/index.md` — implementation plan

## Acceptance Criteria

| #   | Criterion (outcome)                                                 | Verify by                   | Evidence |
| --- | ------------------------------------------------------------------- | --------------------------- | -------- |
| 1   | `resolve_scope()` reads `.cue/HEAD`; falls back to `master`         | unit tests pass             |          |
| 2   | All write paths (`add`, `log`) use `resolve_scope()` not git branch | tests + manual verification |          |
| 3   | `cue switch <slug>` writes HEAD and creates context dir             | integration test            |          |
| 4   | `cue switch master` returns to global context                       | integration test            |          |
| 5   | `cue switch --branch` auto-selects task from git branch             | integration test            |          |
| 6   | `cue status` prints active context (slug or master)                 | manual                      |          |
| 7   | `--task <slug>` flag overrides HEAD for a single invocation         | tests pass                  |          |
| 8   | `cue context` includes active task slug/title/status                | manual                      |          |
| 9   | All existing tests pass                                             | `cargo test`                |          |
