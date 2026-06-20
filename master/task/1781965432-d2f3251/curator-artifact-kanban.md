---
title: "curator: artifact kanban (read-only)"
status: open
priority: normal
---
# curator: artifact kanban (read-only)

Build the `curator` TUI reading `.cue/` artifacts via `cuelib` and rendering
a kanban-style board of tasks, plans, and todos across all registered projects.
No `acuity` involvement at this stage.

## Source

- spec: `.cue/master/spec/curator/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 2)

## Acceptance Criteria

| #  | Criterion                                                              | Verify by                          | Evidence |
| -- | ---------------------------------------------------------------------- | ---------------------------------- | -------- |
| 1  | `curator` launches and displays a kanban board across real projects    | run `curator` in a real project    |          |
| 2  | Tasks, plans, and todos from `.cue/` are visible and correctly grouped | human attestation                  |          |
| 3  | `cue` CLI tests still pass                                             | `cargo test -p cue`                |          |
