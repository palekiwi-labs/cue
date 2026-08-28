---
title: Adapt cue context for tasks
status: complete
priority: high
branch:
  - feat/adapt-context-for-tasks
refs:
  - .cue/master/spec/cue/task-mode.md
---
# Adapt cue context for tasks

The `feat-task-mode` PR (f8d1418) introduced HEAD-based scope resolution via
`resolve_scope()`, but the `gather_context` function used by
`cue context render` was not updated. It still calls `get_current_branch()` to
derive the scope, meaning `cue context render` ignores `.cue/HEAD` and resolves
`context.json` and artifact paths from the sanitized git branch name instead of
the active task slug.

All other `cue context` subcommands (`show`, `profiles`, `path`, `init`)
already use `resolve_scope()` correctly. Only `gather_context` lags behind.

**Note:** The previously proposed idea of injecting active-task orientation
(slug/title/status) into the rendered output has been explicitly abandoned.
Agents that need orientation call `cue status --json` separately. The render
command stays purely artifact-focused.

## Source

- `crates/cue/src/context/mod.rs:170` — `gather_context` calls
  `get_current_branch()` instead of `resolve_scope()`
- `.cue/feat-task-mode/log.md` — Phase 2 history and deferred items
- `.cue/master/spec/cue/task-mode.md` — Part 2 spec

## Acceptance Criteria

| #   | Criterion (outcome)                                                                              | Verify by                                          | Evidence |
| --- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------- | -------- |
| 1   | `gather_context` uses `resolve_scope()` instead of `get_current_branch()`                       | code review: `context/mod.rs`                      | 616fa6c: `get_current_branch` removed from gather_context; replaced with `resolve_scope(&cue_dir)` |
| 2   | `cue context render` resolves artifacts from the active task scope (HEAD), not the git branch   | integration test: render with HEAD set to a slug   | 616fa6c: `test_context_render_uses_head_scope` passes |
| 3   | `cue context render` with no HEAD (or HEAD=master) falls back to the master context             | integration test: render with HEAD absent          | 616fa6c: `test_context_render_no_head_falls_back_to_master` passes |
| 4   | All existing `cue context` tests continue to pass                                               | `cargo test -p cue --test-threads=1`               | 616fa6c: all test suites green (0 failures) |
| 5   | `cargo clippy` reports no new warnings                                                           | `cargo clippy`                                     | 616fa6c: 0 new warnings; 2 pre-existing warnings in switch.rs are out-of-scope |
