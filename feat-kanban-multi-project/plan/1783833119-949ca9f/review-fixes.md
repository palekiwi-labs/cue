---
status: complete
refs:
- .cue/feat-kanban-multi-project/trace/1783828109-dee4aa2/opus-review-kanban-multi-project.md
- .cue/master/task/1783779550-55d53fb/kanban-multi-project.md
- .cue/feat-kanban-multi-project/plan/1783779550-55d53fb/kanban-multi-project.md
---
## Foreword

This plan executes the agreed fixes from the Opus code review of the
`feat/kanban-multi-project` branch (review trace:
`trace/1783828109-dee4aa2/opus-review-kanban-multi-project.md`). It covers one
bug investigation resolved, four direct code fixes, one new master task, and
one decision-required UX refinement.

**M1 — resolved, no code change.** The reviewer's diagnosis was disproven by
reading ratatui 0.29 source (`widgets/list/rendering.rs:41-142`) and by two
regression tests (commit `949ca9f`): List scroll-into-view is driven by
`state.selected` (already set via `list_state.select(Some(sel))` at
`ui.rs:232`), NOT by `highlight_style`/`highlight_symbol`. The selected card is
always visible. Decision: **keep the title-only Cyan highlight as-is.**

The real visual issue the user observed (trailing whitespace at the bottom of
columns when scrolling) is caused by variable card heights, not a scroll bug.
See the decision section (Q1) for the proposed fixed-card-height fix.

**Scope.** Each step is a standalone commit. Green gate for every step:
`cargo test -p curator` + `cargo clippy -p curator -- -D warnings`.

---

## Steps (approved)

### 1. M2 — Drop dead `KanbanTask.project_key`

- [x] Remove the `project_key` field from `KanbanTask` (`app.rs:107-114`).
- [x] Remove the struct-level `#[allow(dead_code)]` (`app.rs:106`) — verify
      clippy stays clean (the field was the only reason it was load-bearing).
- [x] Update `collect_tasks` (`app.rs:692-712`): drop `project_key: key.clone()`
      and the now-unused `key` binding if it becomes unused.
- [x] Update test helpers that construct `KanbanTask`: `make_kanban_task`
      (`app.rs` tests) and `kanban_task_with_title` (`ui.rs` tests).
- [x] Green gate: `cargo test -p curator` + `cargo clippy -p curator -- -D warnings`.

### 2. m1 — Clarify `wrap_title` ellipsis boundary

- [x] Add a comment at `ui.rs:756-765` explaining that `line2` is at most
      `width` chars by construction, so the `>= width` branch only trims when
      line2 exactly fills the width (no real overflow). No behavior change.

### 3. m3 — Wrap the over-length import line

- [x] Break the `use crate::app::{...}` line at `ui.rs:1` across multiple lines
      to stay within the 80-col style. Confirm with `cargo fmt -p curator`.
- [x] Scoped to this one import only — the broader crate-wide rustfmt debt is a
      separate decision (see Q2).

### 4. m4 — Empty-store hint in Red

- [x] Change the trailing `"  |  no projects registered"` span (`ui.rs:836`)
      from `Span::raw(...)` to `Span::styled(..., Style::default().fg(Color::Red))`.
- [x] Existing `kanban_help_line` tests assert via `flatten_line` (substring on
      content, not color) — they still hold; verify.

### 5. m2-task — Create master task for error handling + app.rs refactor

- [x] `cue-task` a new task on **master**: title
      "Curator: surface collect_tasks errors + split app.rs".
- [x] Body captures two out-of-scope items:
      (1) `collect_tasks` (`app.rs:699-702`) silently swallows real IO/parse
      errors per project — surface or log them so a misconfigured project is
      not invisible;
      (2) `app.rs` is 700+ lines — split into modules (e.g. kanban, activity,
      sessions). Also fold in the pre-existing `items_after_test_module` clippy
      lint in `ui.rs` (`--tests` mode; pub(crate) fns defined after the test
      module).
- [x] `refs`: the review trace + the kanban-multi-project task.
- [x] Mark the origin `todo`/note (if any) — none here; this is net-new.

---

## Open Questions (decision required — do NOT execute until answered)

### Q1 — Fixed card height (user proposal) — DECISION REQUIRED

The user identified the real cause of the perceived "scroll bug": trailing
whitespace at the bottom of columns (`cards-scroll-02.txt:8-10` shows three
blank lines). Root cause: **variable card heights** (3 lines for a short
title, 4 lines when the title wraps to 2 lines) interacting with ratatui's
whole-item list rendering — the viewport shows only whole cards, leaving a
variable-height gap that shifts as you scroll over mixed-height cards.

**User's proposal:** give every card a FIXED height by padding the title
block to exactly 2 lines (blank line 2 when the title fits on one). Card
becomes a uniform 4 lines: title-1, title-2-or-blank, project/priority,
blank-pad.

**Recommendation:** implement (option a). Consistent visual rhythm and
predictable scroll jumps.

**Tradeoff:** short-title cards gain one blank line (3 -> 4 lines).

**Limitation:** a fixed height makes trailing whitespace *consistent*
(viewport height mod 4) but cannot fully eliminate it — ratatui shows whole
items only; partial-card display or card stretching is not supported by
default.

**Decision:**
- (a) Implement fixed 4-line cards now (pad short titles to a 2-line block).

**Resolved (commit `442231c`):** Q1 (a) implemented. Short titles padded to a
2-line title block; all cards are now a uniform 4 rows. Test
`render_column_cards_have_uniform_height` verifies project/priority lines
appear at exactly 4-row intervals (rows 3, 7, 11). Awaiting user visual QA.

### Q2 — Crate-wide rustfmt debt (carry-over) — DECISION OPTIONAL

`cargo fmt -p curator` would reformat ~140 pre-existing lines across
`app.rs`, `main.rs`, `sse.rs`, `ui.rs` (pure whitespace/wrapping). Step 3 (m3)
fixes only the one flagged import.

**Decision:**
- (a) Do a separate style commit clearing the crate-wide rustfmt debt
      (recommended — mechanical, clears the debt, independent of feature work).
- (b) Leave it; step 3 only.

---

## Notes

- The scroll-into-view regression tests added in `949ca9f`
  (`render_column_scroll_keeps_{first,last}_selected_visible`) are the
  reviewer's highest-value missing test and stay regardless of the Q1 outcome.
- If Q1 (a) is chosen, it becomes step 6 and should include a test asserting
  uniform card height (e.g. every card renders exactly 4 lines via the
  TestBackend helper).
