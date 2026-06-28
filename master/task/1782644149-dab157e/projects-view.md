---
title: 'Curator: Projects view and cross-project filter'
status: open
priority: normal
---
# Curator: Projects view and cross-project filter

Implement the Projects view (key 1) showing all registered projects with
their harness and last-event timestamp. Wire selection to a narrowing filter
on Activity and Diagnostics.

Depends on: navigation-view-renumber task.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F4)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 8)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | Projects view renders all `ProjectStore` entries | manual with registered projects | |
| 2 | Projects sorted by last-event timestamp descending; zero-event projects at bottom | unit test (sort ordering) | |
| 3 | Zero-event project entry renders with `—` for harness and timestamp — no panic | unit test | |
| 4 | Space/Enter toggles selection on the highlighted project | manual | |
| 5 | Key `a` selects all when any are deselected; deselects all when all are selected | manual | |
| 6 | Activity and Diagnostics show only matching `project_dir` events when filter is active | manual | |
| 7 | Zero projects selected = no filter (all events shown) | unit test | |
| 8 | `ProjectStore` load failure is non-fatal (empty store, warning in status bar) | unit test | |
| 9 | Kanban view shows a one-line hint that the project filter does not apply to it | manual | |
| 10 | All curator workspace tests pass | `cargo test -p curator` | |
