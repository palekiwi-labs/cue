---
status: closed
priority: normal
refs:
- .cue/master/task/task-workflow-phase-2.md
- .cue/master/spec/cue/task-mode.md
---
# Inject active task orientation into `cue context` render

## Context

The task-mode master plan (`feat/task-mode` `plan/index.md`) originally listed
"Update `cue context`: inject active task slug, title, and status into the
rendered context so agents are immediately oriented." This was carried into the
Phase 2 task card as acceptance criterion #8 but was never broken into an
executive step, so it was not implemented.

Removed from the `task-workflow-phase-2` task card during Phase 2 close-out
because it is out of scope for the current PR and warrants a closer look at
what `cue context` actually does before changing its output contract.

## What exists today

`cue context render` (`crates/cue/src/commands/context.rs:56-78`) streams
artifacts and an optional `<instructions>` block. It does NOT emit any active
task / HEAD orientation. The render path uses `gather_context` which is
unaware of `.cue/HEAD`.

## Proposal to investigate

- Decide WHERE task orientation belongs: a header block before artifacts, an
  `<active-task>` element, or injected into the instructions block.
- Decide the shape for the global-context case (HEAD absent / `master`).
- Consider whether `cue status --json` already covers this need and whether
  context render should stay purely artifact-focused.
- Revisit the `gather_context` signature to thread HEAD/scope info through
  without coupling the renderer to the scope module unnecessarily.

## Refs

- `.cue/master/task/task-workflow-phase-2.md` (criterion removed)
- `crates/cue/src/commands/context.rs:56-78` (`handle_render`)
- `.cue/master/spec/cue/task-mode.md` (Part 2, `cue context` section)