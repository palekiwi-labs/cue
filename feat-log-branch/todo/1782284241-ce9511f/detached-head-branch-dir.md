---
status: open
priority: low
---
# Detached HEAD silently writes to `HEAD/` branch directory

## Context

`git::get_current_branch` calls `git rev-parse --abbrev-ref HEAD`. In detached
HEAD state this command succeeds and returns the literal string `"HEAD"` rather
than erroring. As a result, `cue add`, `cue log add`, and `cue log list`
(without `--branch`) silently use `HEAD` as the branch name and operate on a
`.cue/HEAD/` directory.

This is surprising to users who may be in a detached HEAD state (e.g., during
a rebase, bisect, or after `git checkout <sha>`).

## Proposed fix

In `git::get_current_branch` (or at each call site), detect the `"HEAD"` return
value and either:
  a. Return an error: "Cannot determine branch name: HEAD is detached. Use
     --branch to specify a branch explicitly."
  b. Or at minimum document the behaviour as intentional.

Option (a) is safer. It prevents silent data routing to a misleading directory.

## Affected callers (as of feat/log-branch)

- `crates/cue/src/add/mod.rs:51`
- `crates/cue/src/log/mod.rs:57`
- `crates/cue/src/commands/log.rs:63`
- `crates/cue/src/list/mod.rs:196`
- `crates/cue/src/context/mod.rs:170, 239`
