---
title: 'Curator: surface collect_tasks errors + split app.rs'
status: open
priority: normal
refs:
- .cue/feat-kanban-multi-project/trace/1783828109-dee4aa2/opus-review-kanban-multi-project.md
- .cue/master/task/1783779550-55d53fb/kanban-multi-project.md
---
# Curator: surface collect_tasks errors + split app.rs

Two out-of-scope items surfaced during the Opus code review of the
`feat/kanban-multi-project` branch (review trace:
`trace/1783828109-dee4aa2/opus-review-kanban-multi-project.md`).
Tracked here so they are not lost; to be addressed in a future PR.

## Source

- Opus review trace (m2 item)
- `kanban-multi-project.md` task (the work that surfaced these items)

## Acceptance Criteria

| #   | Criterion (outcome) | Verify by | Evidence |
| --- | ------------------- | --------- | -------- |
| 1   | `collect_tasks` no longer silently swallows real IO/parse errors per project — a misconfigured project root is surfaced (logged or shown) rather than invisible | `cargo test -p curator` (new test for error path) + manual | |
| 2   | `app.rs` split into cohesive modules (e.g. kanban, activity, sessions) so no single file exceeds a maintainable size | `cargo test -p curator` + `cargo clippy -p curator -- -D warnings` | |
| 3   | Pre-existing `items_after_test_module` clippy lint in `ui.rs` resolved (pub(crate) fns at ui.rs:1386-1469 are defined after the test module at ui.rs:854) | `cargo clippy -p curator --tests -- -D warnings` (exit 0) | |

## Context

### 1. collect_tasks error swallowing

`collect_tasks` (`app.rs:689-709`) uses `Err(_) => continue` at the
`read_artifacts` call site (`app.rs:696-698`). `read_artifacts` already
returns `Ok([])` for a genuinely missing directory, so an `Err` indicates a
real IO or parse failure (permissions, corrupt frontmatter, etc.). Currently
such a project is silently invisible on the kanban — the user sees fewer
cards than expected with no explanation.

Surface or log these errors so a misconfigured project is not invisible.
Options to evaluate: log to stderr, aggregate into a diagnostics surface, or
return a `Result` with collected per-project errors.

### 2. app.rs size + items_after_test_module lint

`app.rs` is 1611 lines — large enough that navigation and cognitive load
suffer. Split into modules (e.g. `kanban`, `activity`, `sessions`, `app`
core). This refactor should also fold in the pre-existing
`items_after_test_module` clippy lint in `ui.rs`: seven pub(crate) fns
(`format_datetime_on`, `harness_abbrev`, `trunc_pad`, `format_tokens`,
`format_event_datetime`, `session_unique_agents`, `session_unique_models`)
are defined at `ui.rs:1386-1469`, AFTER the `#[cfg(test)] mod tests` block
that starts at `ui.rs:854`. This lint fires only under `--tests` mode (the
project green gate omits `--tests` so it does not block today), but should be
resolved as part of the module split.