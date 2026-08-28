---
title: 'Curator: multi-project kanban with cards + detail pane'
status: complete
priority: high
refs:
- .cue/feat-kanban-multi-project/plan/1783779550-55d53fb/kanban-multi-project.md
---
# Curator: multi-project kanban with cards + detail pane

Make the curator kanban useful for daily multi-project practice. Collect task
cards from all registered projects (CWD-independent), render richer multi-line
cards, and add an Enter-toggled bottom detail pane.

Cross-project *filtering* is explicitly deferred to a later PR (the existing
`projects-view.md` task / Slice 8).

## Source

- spec: `spec/curator/curator-improvements.md` (multi-project kanban, formerly
  out-of-scope line 80, now in scope)
- plan: `plan/<ts>/kanban-multi-project.md`

## Design decisions (locked)

| # | Decision |
|---|----------|
| D1 | Remove `--root` entirely; kanban is always global via `ProjectStore`. |
| D2 | Card label = basename of project root; detail pane = full path. |
| D3 | Read first path only per project key (avoids duplicate cards). |
| D4 | Detail pane is reflective-only; Enter toggles it; j/k navigates columns. |
| D5 | Title char-wrap, max 2 lines, `…` ellipsis. |

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---------------------|-----------|----------|
| 1 | Kanban reads tasks from every registered `ProjectStore` project, ignoring CWD | unit test (CUE_DATA_DIR + temp dirs) | `collect_tasks_multi_project` pass |
| 2 | Missing/inaccessible registered roots are skipped without error | unit test | `collect_tasks_skips_missing_root` pass |
| 3 | Only the first path per project key is read | unit test | `collect_tasks_uses_first_path_only` pass |
| 4 | Each card is a multi-line entry: wrapped title (max 2 lines, `…` on overflow) + priority + project basename | unit test (wrap_title) + manual | `wrap_title_*` (8 tests) pass; manual card QA attested by user 2026-07-12 |
| 5 | Cards are sorted by priority within each column (critical→high→normal→low) | unit test | `priority_sort_in_app_new_is_critical_high_normal_low` pass |
| 6 | `--root` flag removed; kanban launches from any directory with the same content | manual | attested by user 2026-07-12 |
| 7 | Enter toggles a bottom detail pane showing full title, full project path, status | manual | attested by user 2026-07-12 |
| 8 | Empty project store shows a status-bar hint instead of an empty crash | unit test | `kanban_help_line_empty_shows_hint` + `kanban_help_line_non_empty_omits_hint` pass |
| 9 | All curator workspace tests pass | `cargo test -p curator` | 124 passed; 0 failed; exit 0 (2026-07-12) |
| 10 | Clippy clean | `cargo clippy -p curator -- -D warnings` | exit 0, no warnings (2026-07-12) |
