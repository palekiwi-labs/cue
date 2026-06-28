---
title: 'Curator: activity view redesign and fold state'
status: open
priority: high
---
# Curator: activity view redesign and fold state

Replace the flat event iteration in `render_activity` with rendering driven by
`build_activity_items`. Add fold state to `App`. Selection tracks logical item
identity instead of a raw visual index.

Depends on: activity-item-model task.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F2)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 6)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `App` has `fold_state: HashSet<(String, String)>` (session_id, turn_id); starts empty | code review | |
| 2 | `App` selection tracks logical identity (not raw visual index) | code review | |
| 3 | Default view: only session headers and turn summary lines rendered (all folded) | manual demo | |
| 4 | Toggling fold on a turn reveals its tool calls indented 4 spaces | manual demo | |
| 5 | Session-context headers use format `── harness  project_name  (session title)` | manual demo | |
| 6 | `session_idle` events render as standalone rows (not inside a turn) | unit test | |
| 7 | Selection survives a fold/unfold operation (stays on same logical item) | unit test | |
| 8 | Selection survives a new SSE event arriving (no drift onto a different row) | unit test | |
| 9 | All curator workspace tests pass | `cargo test -p curator` | |
