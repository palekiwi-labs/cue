# Code Review — curator Slice 2 TUI Kanban Board

Reviewers: diff-reviewer-sonnet + consultant-opus
Commit reviewed: 4dace76 (feat: add curator TUI kanban board)

---

## [high] Terminal not restored on panic

**Files:** `crates/curator/src/tui.rs`, `crates/curator/src/main.rs:36-40`

If `run()` panics, `tui::restore()` is never called and the user's shell is
left in raw mode / alternate screen. Both reviewers recommend either a panic
hook or a RAII `Drop` guard on `Tui`. The panic hook pattern is simplest:

```rust
let default = std::panic::take_hook();
std::panic::set_hook(Box::new(move |info| {
    let _ = tui::restore();
    default(info);
}));
```

**Status:** RESOLVED — commit 66064a2

---

## [high] Silent task loss for unknown/missing status

**File:** `crates/curator/src/app.rs:62-72`

The `_ => {}` catch-all silently drops tasks with a missing, typo'd, or
`closed` status. This duplicates logic already authoritative in `cuelib`:
`ArtifactMeta::status::<TaskStatus>()` + `TaskStatus::is_kanban_visible()`
already handle exactly this classification. Routing `App::new` through those
typed APIs would fix the stringly-typed duplication and make filtering
intentional and visible.

**Status:** RESOLVED — commit 66064a2

---

## [medium] `should_quit` is dead code

**Files:** `crates/curator/src/app.rs:55`, `crates/curator/src/main.rs:55-57`

Never set to `true`; either remove it or make it the single quit path.

**Status:** RESOLVED — removed in commit 66064a2

---

## [medium] `event-stream` feature unused

**File:** `crates/curator/Cargo.toml:9`

Pulls in `futures-core` for nothing; drop the feature, use plain
`crossterm = "0.28"`.

**Status:** RESOLVED — commit a476e56

---

## [medium] Two separate `io::stdout()` handles in `tui::init()`

**File:** `crates/curator/src/tui.rs`

`execute!` flushes to one handle, `CrosstermBackend` writes through another.
Capture a single handle and pass it to both.

**Status:** RESOLVED — commit a476e56

---

## [medium] `env::current_dir().expect(...)` panics instead of `?`

**File:** `crates/curator/src/main.rs:35`

`main` already returns `Result`; use `.context("cannot read cwd")?`.

**Status:** RESOLVED — commit 66064a2

---

## [medium] `tui::restore()` error swallows `run()` error

**File:** `crates/curator/src/main.rs:38`

`restore()?` early-returns on restore failure, silently discarding the run
error. Fix the sequencing.

**Status:** RESOLVED — commit 66064a2

---

## [medium] Empty-state UX

If `.cue/master/task/` does not exist, the board is silently empty with no
hint to the user.

**Status:** OPEN

---

## [low] `App` fields all `pub`

`sel_*` should be private to protect the in-bounds invariant.

**Status:** OPEN

---

## [low] `Column::left/right` clamping at boundaries is correct

Both reviewers agree — no wrap. Keep as-is.

**Status:** RESOLVED (no change needed)

---

## [low] `Action::None` — consider `Option<Action>`

More idiomatic "no event" representation.

**Status:** OPEN

---

## [low / nit] `task.title.clone()` in `ui.rs`

Borrow `&task.title` / `as_str()` instead.

**Status:** RESOLVED — commit a476e56

---

## [nit] `area: ratatui::layout::Rect` in `render_column` signature

Add `Rect` to the `use` block.

**Status:** RESOLVED — commit a476e56

---

## [medium] Per-column `match active_col` triplication

`[ColumnState; 3]` array would kill the boilerplate (Opus recommendation).

**Status:** OPEN

