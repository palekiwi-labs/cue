---
title: 'Curator: ActivityItem pure build function'
status: complete
priority: high
---
# Curator: ActivityItem pure build function

Define the `ActivityItem` enum and implement the `build_activity_items` pure
function that transforms the events ring buffer into a structured rendering model.

Depends on: db-eventrecord-enrichment task (needs `project_dir` + `harness` on `EventRecord`).

This is the highest-risk slice. Over-test it.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F2)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 5)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `ActivityItem` enum defined with `SessionHeader`, `Turn`, and `Standalone` variants | code review | `activity.rs:17-31` — all 3 variants defined |
| 2 | `build_activity_items` is a pure function (no side effects, no App mutation) | code review | `activity.rs:75` — `pub fn`, takes `&`/`&VecDeque`/`&HashMap`, no `&mut App` |
| 3 | Tool-call grouping keyed on `(session_id, turn_id)` via HashMap — not positional adjacency | code review + test | `activity.rs:83-90` — `turn_map: HashMap<(&str, &str), (usize, bool)>` |
| 4 | Two interleaved sessions with the same `turn_id` do not cross-contaminate | unit test | `two_sessions_same_turn_id_no_cross_contamination` (`activity.rs`) |
| 5 | Orphan tool call (parent turn evicted from ring buffer) renders as `Standalone`, no panic | unit test | `orphan_tool_call_renders_as_standalone` (`activity.rs`) |
| 6 | Tool call arriving by `seq` before its `agent_turn_completed` still groups correctly | unit test | `tool_call_lower_seq_than_turn_still_groups` (`activity.rs`) |
| 7 | Session-context header emitted on `project_dir` change within the same `session_id` | unit test | `project_dir_reentry_emits_third_header` (`activity.rs`) |
| 8 | Empty `fold_state` → tool calls counted but not included in item list (all folded) | unit test | `empty_fold_state_all_turns_have_empty_tool_calls` (`activity.rs`) |
| 9 | Non-empty `fold_state` → matching turn includes its tool calls | unit test | `fold_state_entry_expands_that_turn_only` (`activity.rs`) |
| 10 | `push_event` sets `SessionSummary.project_dir` from every event type (not only `SessionIdle`) | unit test | `project_dir_set_from_non_idle_first_event` (`app.rs`) |
| 11 | All curator workspace tests pass | `cargo test -p curator` | 60 passed (commits ae09f03, 3976d5c, 0c2ff37) |
