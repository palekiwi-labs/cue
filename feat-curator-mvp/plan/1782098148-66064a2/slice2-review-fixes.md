---
status: complete
---
# Slice 2 — Review Follow-up (quick wins)

## Foreword

This plan addresses the four low-risk findings from the Sonnet + Opus code
review of commit 4dace76, as triaged in the follow-up discussion. All four
changes are in `crates/curator/` only. Base commit: 66064a2.

The two high-severity items were already fixed in 66064a2. This plan covers
only the four "fix now" items agreed upon in triage.

No new behaviour is introduced. All existing tests must remain green.
`cargo clippy -p curator -- -D warnings` must pass after all changes.

## Steps

- [x] 1. `crates/curator/Cargo.toml` — drop `features = ["event-stream"]` from
       the `crossterm` dependency; use plain `crossterm = "0.28"`.

- [x] 2. `crates/curator/src/tui.rs` — capture a single `io::stdout()` handle
       in `init()` and pass it to both `execute!` and `CrosstermBackend::new`.
       Do NOT change the `restore()` function (it is intentionally stateless
       for use in the panic hook).

- [x] 3. `crates/curator/src/ui.rs` — in the `render_column` item-building
       closure, change `Span::raw(task.title.clone())` to
       `Span::raw(task.title.as_str())` to avoid a per-frame heap allocation.

- [x] 4. `crates/curator/src/ui.rs` — add `Rect` to the existing
       `ratatui::layout` use block and change the `render_column` function
       signature from `area: ratatui::layout::Rect` to `area: Rect`.

## Verification

Run after all four changes:

```
cargo build -p curator
cargo clippy -p curator -- -D warnings
cargo fmt --check -p curator
cargo test -p cue -p cuelib
```

All must pass with zero warnings/errors before handing back.

## Out of scope

- `[ColumnState; 3]` refactor (deferred to next slice)
- `App` field visibility tightening (deferred, blocked on above)
- `Option<Action>` instead of `Action::None` (deferred)
- Empty-state UX (separate task)
- `Column::left/right` — no change (correct behaviour confirmed)
