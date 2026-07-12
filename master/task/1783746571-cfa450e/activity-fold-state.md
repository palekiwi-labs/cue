---
title: 'Curator: activity fold state and toggle (stage C)'
status: open
priority: low
refs:
- .cue/master/task/1782644149-dab157e/activity-view-redesign.md
- .cue/master/task/1782644149-dab157e/activity-feed-rendering.md
- .cue/feat-curator-activity-item/spec/index.md
---
# Curator: activity fold state and toggle (stage C)

Add fold-state to `App` and wire `build_activity_items` into the render loop,
replacing the current flat filtered-set rendering. Selection tracks logical
item identity so the highlight survives fold/unfold and new SSE arrivals.

Split out from the closed `activity-view-redesign.md` task. The rendering
legibility work that landed in Slice 6b is tracked in
`activity-feed-rendering.md`.

This is **stage C** territory and also includes the nested `parent_id` tree
(folding child sessions under parent turns). The current
`build_activity_items` header key `(session_id, project_dir)` is degenerate
(`project_dir` is workspace-constant) and will be reworked with
`parent_id`-based nesting as part of this work.

## Source

- original task: `activity-view-redesign.md` (closed — criteria 1, 3, 4, 7)
- plan (deferred): `plan/1782659497-0c2ff37/slice6b-rendering.md` "Out of scope"
- investigation: `spec/log.md` ("Slice 6b design locked" — `build_activity_items`
  stays unwired; stage C rewrites with `parent_id`-based nesting)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `App` has `fold_state: HashSet<(String, String)>` (session_id, turn_id); starts empty | code review | |
| 2 | `build_activity_items` is called from the render loop (replaces flat filtered iteration) | code review | |
| 3 | Default view: only session headers and turn summary lines rendered (all folded) | manual demo | |
| 4 | Toggling fold on a turn reveals its tool calls indented | manual demo | |
| 5 | Selection survives a fold/unfold operation (stays on same logical item) | unit test | |
| 6 | Nested tree: child sessions fold under their parent's Task-tool-completed turn | manual demo | |
| 7 | All curator workspace tests pass | `cargo test -p curator` | |

## Trigger

No fixed trigger. Action when the flat reverse-chrono layout becomes a
legibility bottleneck (e.g. many concurrent sub-agent sessions), or when
persistent session summaries land (depends on sessions-table normalization
todo).
