# Curator Specification

## References

- [kanban ref analysis](.cue/master/doc/1781710490-3e2d075/kanban-ref-analysis.md)

## Purpose

`curator` is a TUI dashboard for the cue ecosystem. It provides:

- A kanban-style view of cue artifacts (tasks, plans, todos) read from the
  local `.cue/` directory via `cuelib`.
- A live activity view of agent events sourced from `acuity`.

The goal is a single-pane dashboard showing the state of all projects, current
agent activity, and accumulated observability data (e.g. token counts per task,
session idle state).

## Crate Dependencies

- `cuelib`: reads `.cue/` artifact state and the project registry.
- `acuity-api`: read/response types for `acuity`'s SSE stream and query
  endpoints.

## Data Sources

| Source | What it provides |
| --- | --- |
| `.cue/` local filesystem (via `cuelib`) | Task/plan/todo state, project registry |
| `acuity` SSE stream | Real-time agent lifecycle events |
| `acuity` query API | Historical event data |

## Deferred

- Detailed UI layout and component design.
- Specific acuity query patterns.
- Connection config (acuity host/port).
