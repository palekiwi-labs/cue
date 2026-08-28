---
status: closed
title: Support --task flag with cue context
refs:
  - .cue/master/task/worktree-store-and-task-env-impl.md
---
`cue context render` and `cue context show` should accept an optional `--task`
flag that uses the context.json file from that task context dir

Superseded: absorbed into task `worktree-store-and-task-env-impl` (plan
Phase 6). The `$CUE_TASK` precedence chain being built there already
requires the `--task` flag rung for `cue context` (the `(flag)`
provenance label cannot exist without it), and the flag shares the same
scope-resolution code path — implementing it separately would touch the
same code twice.
