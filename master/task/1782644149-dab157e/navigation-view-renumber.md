---
title: 'Curator: navigation enhancements and view renumber'
status: open
priority: normal
---
# Curator: navigation enhancements and view renumber

Add fold/unfold, Home/End, and PgUp/PgDn key bindings. Renumber views to
1=Projects, 2=Kanban, 3=Activity, 4=Diagnostics. Update all help-bar strings.

Depends on: activity-view-redesign task.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F3, F4 keybindings)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 7)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `View` enum has `Projects`, `Kanban`, `Activity`, `Diagnostics` variants | code review | |
| 2 | Keys 1/2/3/4 switch to the correct views | manual | |
| 3 | Space toggles fold on the selected Activity turn | manual | |
| 4 | Left/Right toggle fold in Activity view; Left/Right navigate columns in Kanban | manual | |
| 5 | Home scrolls to top; End scrolls to bottom of the current view | manual | |
| 6 | PgUp/PgDn scroll by page height, clamped at list bounds | unit test (nav-math) | |
| 7 | All three help-bar strings updated to reflect current keybindings | code review | |
| 8 | No new action variant is swallowed silently by `Action::None` | code review | |
| 9 | All curator workspace tests pass | `cargo test -p curator` | |
