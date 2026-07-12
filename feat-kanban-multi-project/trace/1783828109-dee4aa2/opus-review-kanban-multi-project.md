# Code Review: feat/kanban-multi-project

**Reviewer:** claude-opus-4-8
**Base:** master (merge base 55d53fb)
**Scope:** Multi-project kanban redesign for the `curator` CLI (ratatui 0.29 TUI).
Reviewed the full branch diff and read `app.rs`, `main.rs`, `ui.rs`, plus the
underlying `cuelib` APIs (`ProjectStore`, `read_artifacts`, `ArtifactMeta`).

Overall this is a well-structured, well-tested change that mirrors the existing
`ActivityLayout` pattern faithfully. Findings below, ordered by severity.

---

## Critical

None.

---

## Major

### M1 - Selected card can render outside the visible column (no scroll offset with multi-line items)
`crates/curator/src/ui.rs:226-235`

Cards are now 4 visual rows each (up to 2 title lines + project/priority + blank
pad), but the highlight was removed (`highlight_symbol` gone, `highlight_style`
gone) and only `list_state.select(Some(sel))` remains. With ratatui's `List`,
the automatic scroll-into-view behavior is driven by the highlight. Now that
there is no `highlight_style`/symbol and cards span multiple rows, a selection
deep in a long column may sit below the fold with no way to scroll it into view
- `j`/`k` update `sel` but the viewport won't follow because the list has no
visible selection cue and, more importantly, tall items amplify how quickly you
run off-screen.

Please verify manually: create a project with ~15 open tasks, press `j` to the
bottom, and confirm the selected card scrolls into view. ratatui's `ListState`
does still track offset even without a highlight symbol, but multi-line
`ListItem`s combined with `HighlightSpacing::Never` are a known source of
off-by-viewport surprises. If it does not follow, you'll want to keep
`.highlight_style(...)` (even a subtle one) or manage the offset explicitly.

### M2 - `KanbanTask.project_key` is dead + `#[allow(dead_code)]` masks it
`crates/curator/src/app.rs:106-114`, `app.rs:706`

`project_key` is populated in `collect_tasks` but never read anywhere in
rendering or detail (the detail pane uses `project_root`, and the card uses
`project_root.file_name()`). The `#[allow(dead_code)]` on the whole struct
suppresses the clippy signal that would otherwise flag it. Given the `-D warnings`
gate, this allow is load-bearing only because the field is unused.

Two options:
- If `project_key` is intended for a future slice, add a `// TODO(slice-N)` note
  explaining why it's carried, and prefer a field-level `#[allow(dead_code)]`
  over struct-level so genuinely-dead future additions aren't silently hidden.
- If it isn't needed, drop it (and the `key.clone()` in `collect_tasks`), which
  also removes an allocation per task.

Struct-level `#[allow(dead_code)]` is the kind of thing that hides real dead
code later - worth narrowing.

---

## Minor

### m1 - `wrap_title` ellipsis boundary is subtle (not a real overflow)
`crates/curator/src/ui.rs:756-765`

When `line2` fits within `width` (the `else` branch at line 761-762), the code
appends the ellipsis unconditionally without checking `line2.len() + 1 <= width`.
Tracing the reachable cases shows line2 is at most `width` chars by construction
(the loop only emits chunks of `width` or word-accumulations `<= width`), so
there is no genuine overflow. However the `>= width` condition is subtle and only
reachable when line2 exactly fills the width. A short clarifying comment ("line2
is at most `width` chars by construction; only trim when it fully fills the
width") would prevent a future maintainer from thinking it's buggy. No code
change strictly required.

### m2 - `read_artifacts` errors are silently swallowed per project
`crates/curator/src/app.rs:699-702`

`Err(_) => continue` discards the error entirely. For a multi-project board, a
single mis-configured project (e.g. a permissions error, not just a missing dir)
will make that project's tasks silently vanish with zero user feedback. The doc
comment says "missing or unreadable are silently skipped," so this is intentional
per D3, but since `read_artifacts` already returns `Ok(vec![])` for a
non-existent directory (`artifact.rs:244` + `collect_files` guards `is_dir`), the
`Err` arm here fires only on real IO/parse errors - exactly the cases worth not
hiding forever. A `todo` note or a debug-log hook would be reasonable. Not
blocking.

### m3 - `ui.rs` use-line exceeds project 80-col style
`crates/curator/src/ui.rs:1`

```rust
use crate::app::{AcuityStatus, ActivityLayout, App, Column, KanbanLayout, KanbanTask, SessionSummary, View};
```
This is ~104 cols. Project style targets <=80 (132 only when it significantly
improves readability). `rustfmt` would normally break this into a multi-line
import; worth running `cargo fmt` to confirm the imports get wrapped.

### m4 - Empty-store hint text is inconsistent with the help-bar grammar
`crates/curator/src/ui.rs:846-848`

The hint `"  |  no projects registered"` is rendered with `Span::raw` (default
fg), so it visually blends with the dim help separators rather than reading as an
actionable state. Given the intent (self-explanatory empty board), consider
styling it (e.g. `Color::Yellow` or `DarkGray`) to distinguish it from key hints,
and/or phrasing it as guidance (`no projects registered - run
`cue project add``). Purely a UX polish call.

---

## Nits

### n1 - `collect_tasks` iteration order is deterministic but undocumented
`crates/curator/src/app.rs:692-712`

`ProjectStore.entries()` returns a `BTreeMap` (`project.rs:74,139`), so iteration
is key-sorted and stable. The subsequent `classify_tasks` sort is only by
`priority_rank`, and Rust's `sort_by_key` is stable, so within a priority band
tasks retain project-key order then `read_artifacts` path-sorted order
(`artifact.rs:248`). This is a nice deterministic property worth a one-line
comment in `collect_tasks`, since a future switch of `entries()` to a `HashMap`
would silently make the board order jitter frame-to-frame.

### n2 - Test helper duplication
`crates/curator/src/app.rs:330-338` and `373-378`

`write_task` is defined twice (once in `collect_tasks_multi_project`, once in
`collect_tasks_uses_first_path_only`) with near-identical bodies. Minor test-code
duplication; a single module-level helper would DRY it up. Low value given
they're test-local.

### n3 - `\u{2014}` magic literal for the status placeholder
`crates/curator/src/ui.rs:96`

`t.meta.status_raw.as_deref().unwrap_or("\u{2014}")` uses an em-dash for "no
status." Fine, but a named `const EM_DASH` or a comment would aid readability,
consistent with how `\u{2026}` is documented in `wrap_title`.

---

## Test Quality Assessment

Strong coverage overall:
- `wrap_title`: empty, zero-width, exact-width, single-word char-break,
  multi-word wrap, and ellipsis truncation - good boundary coverage.
- `collect_tasks`: multi-project, missing-root skip, first-path-only (D3),
  empty-store - the important paths.
- `kanban_help_line`: empty/non-empty hint and layout-driven Enter text.
- `KanbanLayout` toggle both directions.

Gaps worth considering (not blocking):
1. `collect_tasks` with a `Closed`/unrecognised-status task - no test confirms
   `collect_tasks` returns the raw meta (classification happens later in
   `classify_tasks`); an end-to-end test that a closed task is
   collected-but-not-columned would lock the contract.
2. `wrap_title` with leading/trailing/internal multiple spaces - `split_whitespace`
   collapses them, which is correct but untested; a title like `"  a   b  "`
   silently normalizes.
3. `render_column` scroll-into-view (see M1) - there's no rendering test asserting
   the selected multi-line card is visible; this is the highest-value missing
   test given the highlight removal.
4. `collect_tasks` when a project key has an empty path vec - the `paths.first()`
   `None => continue` arm (`app.rs:696-697`) is unreachable via `add_path` (which
   never creates empty vecs) but is defensive; a direct-construction test would
   cover it.

---

## Summary

| Severity | Count | Headline |
|----------|-------|----------|
| Critical | 0 | - |
| Major | 2 | Scroll-into-view for multi-line cards (M1); dead `project_key` + broad `dead_code` allow (M2) |
| Minor | 4 | Ellipsis-width comment (m1), swallowed IO errors (m2), 80-col import (m3), empty-store hint styling (m4) |
| Nit | 3 | Determinism comment (n1), test helper dup (n2), magic literals (n3) |

The two Major items are the ones to resolve before merge - M1 because it's a
potential functional regression (selection running off-screen after the highlight
removal), and M2 because the struct-level `#[allow(dead_code)]` interacts with the
`-D warnings` gate to hide an unused field that either needs a purpose or removal.
Everything else is discretionary polish. The TDD discipline and the faithful
mirroring of `ActivityLayout` are commendable.
