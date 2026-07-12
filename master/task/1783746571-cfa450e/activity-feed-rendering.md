---
title: 'Curator: activity feed rendering legibility (Slice 6b)'
status: complete
priority: high
refs:
- .cue/feat-curator-activity-item/plan/1782659497-0c2ff37/slice6b-rendering.md
- .cue/feat-curator-activity-item/spec/index.md
- .cue/master/task/1782644149-dab157e/activity-item-model.md
---
# Curator: activity feed rendering legibility (Slice 6b)

Make the activity feed legible by consuming the Slice 6a `SessionSummary`
fields (title, model, parent_id, harness) in the flat reverse-chrono
rendering layer. Retains the flat layout; stage C owns the nested
`parent_id` tree.

This task supersedes the rendering portion of `activity-view-redesign.md`
(which is closed). The fold-state/toggle work is tracked separately in
`activity-fold-state.md`.

Branch: `feat-curator-activity-item`

## Source

- plan: `plan/1782659497-0c2ff37/slice6b-rendering.md` (steps 1-6, 8 complete)
- design decisions: `spec/log.md` ("Slice 6b design locked" entry)

## What shipped (6 commits)

- `session_label` helper — title or dim id-suffix placeholder (last ~8 chars,
  not the buggy first 8). Pins the `ui.rs:214` prefix-collision fix.
- `is_hidden_in_activity` + `App::activity_len` — hides `session_updated`
  rows; single source of truth for the hide rule.
- `scroll_down_activity` clamp — bound changed from `events.len()` to
  `activity_len()` (correctness keystone).
- `activity_block_title` — harness + project basename once in the block title.
- rebuilt `render_activity` — filtered-set iteration, sel computed once and
  clamped, conditional header style (DarkGray placeholder / Cyan+Bold title).
- per-turn model appended in `event_summary` when `Some`.

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | Prefix-sharing session ids render distinct headers (suffix, not prefix) | unit test | `session_label_prefix_sharing_ids_are_distinct` (`ui.rs`) |
| 2 | Header shows title when known, dim id-suffix placeholder otherwise | unit test | `session_label_*` tests (`ui.rs`) |
| 3 | `session_updated` rows hidden from the feed | unit test | `activity_len_excludes_hidden_session_updated` (`app.rs`) |
| 4 | Selection highlight never vanishes/jitters when scrolling | unit test | `scroll_down_activity_clamps_at_activity_len_not_events_len` (`app.rs`) |
| 5 | Per-turn model appended cleanly when `Some`, omitted (no dangling sep) when `None` | unit test | `event_summary_agent_turn_completed[_with_model]` (`ui.rs`) |
| 6 | Block title shows harness + project basename once | unit test | `activity_block_title_*` tests (`ui.rs`) |
| 7 | All curator tests pass | `cargo test -p curator` | 60 passed |
| 8 | Live QA: all six rendering properties confirmed visually | manual demo | confirmed by user, 2026-07-11 |
