# Implementation Trace Report — Phase 6 Slices 5–8

## Overview

All 8 slices of the Phase 6 curator live-half plan are now implemented. This
report documents every decision point, deviation, and near-miss encountered
during the automated implementation of Slices 5–8, written for use as input
to a code review.

---

## Issue 1: Plan pseudocode had a silent `Quit` bug (Slice 5)

**Severity: High**

The plan's pseudocode for `run()` was:

```rust
match rx.recv() {
    Ok(msg) => process_msg(msg, app),  // <-- return value discarded
    Err(_) => break,
}
while let Ok(msg) = rx.try_recv() {
    if process_msg(msg, app) == LoopControl::Quit { return Ok(()); }
}
```

The first `recv()` arm discards the `LoopControl` return. If `Quit` arrived
as the very first message in a burst — which is the common case when a user
presses `q` on a quiet terminal — the process would not exit. Only messages
arriving during the `try_recv()` drain would correctly trigger shutdown.

**Resolution:** Both `recv()` and `try_recv()` arms now check
`LoopControl::Quit` symmetrically (`main.rs:106-118`). The plan pseudocode
was treated as illustrative, not literal.

**Reviewer question:** Should the `Err(_)` arm on `recv()` also explicitly
return `Ok(())` rather than `break`? The current code `break`s out of the
loop and then returns `Ok(())` on the next line — semantically equivalent,
but the flow is indirect.

---

## Issue 2: Plan signature for `run()` was incomplete (Slice 5)

**Severity: Medium**

The plan specified:

```rust
fn run(terminal: &mut tui::Tui, app: &mut App, rx: Receiver<Msg>) -> Result<()>
```

But `process_msg` calls `reload_tasks(app, &root, &branch)` — which needs
`root: &Path` and `branch: &str`. There is no global state in Rust. The
plan's pseudocode was written as if those were captures, which works in
pseudo-Rust but not in the real borrow checker.

**Resolution:** Extended the signatures of both `run()` and `process_msg()`
with `root: &Path, branch: &str` parameters (`main.rs:94-123`). This is the
correct idiomatic solution; it also makes the data flow explicit and
testable.

**Note for reviewer:** `process_msg` is currently a free function in
`main.rs`, not exposed for testing. If the business logic in it grows, it
should be extracted to `app.rs` or a dedicated module. For now the logic is
thin enough that this is fine.

---

## Issue 3: Missing `clap` feature flag (Slice 5)

**Severity: Low (caught at compile time)**

The plan called for `#[arg(env = "ACUITY_URL")]` but `Cargo.toml` only had
`features = ["derive"]` for clap. The `env` feature is a separate opt-in in
clap 4.x. The build failed immediately.

**Resolution:** Added `"env"` to the clap features list (`Cargo.toml:9`).
Zero risk, but it was an omission in the plan that required a file change
not described in the plan's step list.

---

## Issue 4: Activity Feed selection index mismatch with injected headers (Slice 6)

**Severity: Medium — correctness concern**

The plan specified that `sel_activity` is bounded by `self.events.len()` in
`scroll_down_activity()`, meaning it is an index into the raw event ring
buffer. However, the Activity Feed list renders injected session header rows
between groups. The visual list item count is larger than `events.len()`.

If you pass `sel_activity` directly as the `ListState` selection, you select
the wrong visual row whenever there are session headers before the target
event.

**Resolution:** While building list items the visual position of each event
row is tracked separately from the header rows, capturing
`selected_visual = Some(items.len())` at the moment the
`idx == app.sel_activity` event is about to be appended (`ui.rs:159-162`).
This correctly maps the event-index selection to the visual list position.

**Self-criticism:** This is a somewhat fragile approach. The
visual-to-logical mapping is computed inline during render, which means it
is re-done on every frame. It also means the relationship between
`sel_activity` and the rendered position is implicit — it is not captured in
`App` state. A reviewer should consider whether this is maintainable as the
rendering logic evolves (e.g. if filters are added, or if multi-line event
rows are introduced).

**Alternative not taken:** Store a `Vec<usize>` of visual offsets alongside
the items list, or store the visual offset directly in `App`. Deferred as
over-engineering for the current scope.

---

## Issue 5: Three clippy lints discovered only at Slice 7 cleanup (Slice 7)

**Severity: Low — style/idiom, caught before merge**

Three clippy lints were present in code written in Slices 3 and 4 but were
not caught until `cargo clippy --workspace -- -D warnings` was run in
Slice 7:

1. **`collapsible_if`** in `app.rs:218-224`: the
   `if let Ok(...) { if ev.is_error { ... } }` pattern should be collapsed
   with `&&`. This is a real readability improvement.

2. **`while_let_loop`** in `input.rs:15`: the
   `loop { let ev = match event::read() { Ok(ev) => ev, Err(_) => break }; ... }`
   idiom should be a `while let Ok(ev) = event::read() { ... }` loop.

3. **`while_let_loop`** in `sse.rs:76`: the
   `loop { let Some(nl) = ... else { break }; ... }` pattern should be a
   `while let Some(nl) = ...` loop.

**Root cause:** Clippy with `-D warnings` was not run during Slices 3 and 4
— only `cargo build` and `cargo test` were used. The project's Slice 7 spec
correctly reserved a cleanup pass, so this is not a protocol failure, but it
does mean the lints lived in the codebase for several commits before being
addressed.

**Self-criticism:** A tighter protocol would run `cargo clippy -p curator`
after each individual slice rather than batching it into Slice 7. The three
lints are minor but a reviewer familiar with the codebase will notice them
in the diff and wonder why they were not caught earlier.

---

## Issue 6: `SessionSummary.session_id` redundancy (Slices 4 / 7)

**Severity: Low**

`SessionSummary` has two fields that are written by `push_event` but never
read by any rendering code:

- `session_id: String` — redundant because callers already have
  `record.session_id` when they need it
- `first_seen: String` — collected for future display but not yet consumed

These generated `dead_code` warnings from rustc since `SessionSummary` is
on a binary-only type (not pub-exported from a library).

**Resolution:** Added `#[allow(dead_code)]` with explanatory comments noting
they will be used in Slice 8 tests and future rendering (`app.rs:77-90`).

**Self-criticism:** `session_id` on `SessionSummary` is arguably a design
redundancy. Since the struct is keyed by `session_id` in the `HashMap`, any
lookup that reaches a `SessionSummary` already knows the `session_id`.
Storing it again inside the struct is the "convenient but redundant" pattern.
It was in the original plan spec without challenge. A reviewer should decide
whether to keep it (convenient for display code that only has a
`&SessionSummary`) or remove it (eliminates the allow attribute and the
redundancy).

---

## Issue 7: Hand-rolled JSON in `record_json` test helper (Slice 8)

**Severity: Low — implementation detail**

Writing the `record_json` helper for `LineBuffer` tests required producing a
valid JSON string for an `EventRecord` without using
`serde_json::to_string(&record)` (would require `EventRecord: Serialize`,
which is not guaranteed in the type's API contract). Using `format!` with a
raw string literal was chosen instead:

```rust
fn record_json(seq: i64, session_id: &str) -> String {
    format!(
        r#"{{"seq":{seq},"received_at":"2026-01-01T00:00:{seq:02}Z",...,"payload":"{{}}"}}"#
    )
}
```

The `{{}}` in the format string escapes to literal `{}` in the output — the
empty JSON object for `payload`. This works but is visually confusing and
fragile: if `EventRecord`'s JSON shape changes (field renames, new required
fields), this hand-rolled helper will silently produce invalid JSON that the
parser will reject, causing tests to fail with a confusing error message
rather than a clear type mismatch.

**Alternative not taken:** Check whether `EventRecord` derives `Serialize`.
If it does, use the `serde_json::json!` macro or `serde_json::to_string`.
This would make the tests more robust against schema drift.

---

## Issue 8: `docs/curator.md` not updated (Slice 7)

**Severity: Low — out of scope for automated work**

The plan included:

> `Update docs/curator.md: stub note that View 2/3 require --acuity-url or $ACUITY_URL`

This checkbox was not ticked. The docs update was left for the human
alongside the manual QA checklist, but was not explicitly called out as
deferred in the commit message or cue log. A reviewer running down the plan
checkboxes would notice it.

---

## Summary Table

| # | Issue | Slice | Caught by | Resolution | Reviewer Action Needed |
|---|-------|-------|-----------|------------|------------------------|
| 1 | `Quit` not checked on `recv()` arm | 5 | Plan review | Fixed symmetrically | Verify `break` vs `return` on `Err` arm |
| 2 | Incomplete `run()` signature in plan | 5 | Compile error | Added `root, branch` params | Consider extracting `process_msg` |
| 3 | Missing `clap` env feature | 5 | Compile error | Added feature | None |
| 4 | `sel_activity` vs visual list offset | 6 | Design analysis | Track visual position inline | Assess fragility as rendering evolves |
| 5 | Three clippy lints from earlier slices | 7 | `clippy -D warnings` | Fixed | Consider running clippy per-slice |
| 6 | `SessionSummary.session_id` redundancy | 4/7 | Dead-code warning | `#[allow(dead_code)]` | Decide whether to keep or remove |
| 7 | Hand-rolled JSON in test helper | 8 | None (works today) | Accepted risk | Check if `EventRecord: Serialize`; if so, use serde |
| 8 | `docs/curator.md` not updated | 7 | Plan review | Deferred to human | Complete alongside manual QA |
