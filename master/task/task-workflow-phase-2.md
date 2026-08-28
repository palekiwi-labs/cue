---
title: "Task workflow — Phase 2: HEAD-driven context directories"
status: complete
priority: high
branch: []
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
| 1   | `resolve_scope()` reads `.cue/HEAD`; falls back to `master`         | unit tests pass             | cuelib: 62 passed (head::tests::resolve_scope_* all ok) |
| 2   | All write paths (`add`, `log`) use `resolve_scope()` not git branch | tests + manual verification | add: 42 passed; log: 7 passed; manual QA confirmed 2026-07-13 |
| 3   | `cue switch <slug>` writes HEAD and creates context dir             | integration test            | switch: 10 passed (switch_human_output_to_task, switch_json_to_task ok) |
| 4   | `cue switch master` returns to global context                       | integration test            | switch: 10 passed (switch_human_output_to_master, switch_json_to_master ok) |
| 5   | `cue switch --branch` auto-selects task from git branch             | integration test            | switch: 10 passed (scalar, inline list, multiline block list all ok) |
| 6   | `cue status` prints active context (slug or master)                 | manual                      | manual QA passed 2026-07-13 (human attestation) |
| 7   | `--task <slug>` flag overrides HEAD for a single invocation         | tests pass                  | add: 42 passed (test_add_with_explicit_branch ok); log: 7 passed |
| 8   | All existing tests pass                                             | `cargo test`                | cuelib: 62 passed; cue integration: 130 passed across all suites (2026-07-13) |
