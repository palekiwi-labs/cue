---
status: complete
---
# Review Fixes — Phase 6 Curator SSE

## Foreword

This plan addresses the five pre-merge findings from the Phase 6 code review
(`trace/1782317957-1c74d35/phase-6-code-review.md`). All five touch only two
files: `crates/curator/src/sse.rs` and `crates/curator/src/main.rs`. No new
dependencies. No new Msg variants.

Findings addressed (in order of execution):

| ID | Severity | Summary |
|----|----------|---------|
| M3 | Medium | `eprintln!` in SSE thread corrupts TUI |
| M1 | Medium | `reqwest::Client` rebuilt on every reconnect |
| H2 | High | SSE `data:` multi-line join missing `\n` per spec |
| C1 | Critical | SSE `send()` blocks on full channel, can starve `q` |
| H1 | High | `Err(_)` arm on `recv()` is unreachable dead code |

Deferred (safe post-merge): M2, L1, L2, L3, L4.

**Relationship to Slice 2 (`limit_history`):** A follow-up todo captures a
server-side `?limit_history=N` parameter that will bound initial history
replay. Once that ships, cold-start backlog no longer flows through the
shared channel and C1's `try_send` becomes a belt-and-suspenders guard for
reconnect bursts and live overload. Until then it is load-bearing — ship this
plan first, Slice 2 second. (See `.cue/feat-curator-w-acuity/todo/` for the
Slice 2 capture.)

**Prerequisites:** All 28 existing tests green on current HEAD.

---

## Step 1 — Remove `eprintln!` from the SSE thread (M3)

**File:** `crates/curator/src/sse.rs`

`eprintln!` from a background thread writes directly to stderr while ratatui
owns the terminal, causing visual corruption. The three call sites are:

- `sse.rs:109` — inside `LineBuffer::feed()` after malformed JSON
- `sse.rs:159` — inside `spawn()` when the tokio runtime fails to build
- `sse.rs:186` — inside `run_loop()` after a stream error

**Changes:**

`sse.rs:109` — remove the `eprintln!` inside `LineBuffer::feed()`. The
caller already receives zero records as the failure signal; no additional
diagnostic is needed for a malformed server row.

```rust
// Before:
Err(e) => {
    eprintln!("sse: malformed event JSON, skipping: {e}");
}

// After:
Err(_) => {
    // Silently skip — zero records returned to caller signals the drop.
}
```

`sse.rs:159` — remove the `eprintln!` in `spawn()`. The subsequent
`tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt: 1 }))` already
surfaces the failure to the UI.

```rust
// Before:
Err(e) => {
    eprintln!("sse: failed to build tokio runtime: {e}");
    let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt: 1 }));
    return;
}

// After:
Err(_) => {
    let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt: 1 }));
    return;
}
```

`sse.rs:186` — remove the `eprintln!` in `run_loop()`. The
`SseStatus::Reconnecting { attempt }` already sent at the top of each loop
iteration surfaces the degraded state to the UI.

```rust
// Before:
Err(e) => {
    eprintln!("sse: stream error (attempt {attempt}): {e}");
    tokio::time::sleep(...).await;
    backoff_ms = next_backoff(backoff_ms);
}

// After:
Err(_) => {
    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
    backoff_ms = next_backoff(backoff_ms);
}
```

- [x] Make the three changes above.
- [x] `cargo clippy -p curator -- -D warnings` clean (removing `e` binding
      may trigger `unused_variables` if the binding is kept).
- [x] `cargo test -p curator` green.

---

## Step 2 — Hoist `reqwest::Client` to `run_loop` (M1)

**File:** `crates/curator/src/sse.rs`

`reqwest::Client::new()` at `sse.rs:199` builds a new TLS stack and
connection pool on every reconnect. The client is `Clone` and internally
`Arc`'d — it is designed to be built once and reused.

**Changes:**

Move client construction from `connect_and_stream` into `run_loop`, pass
`&reqwest::Client` down:

```rust
// run_loop: add one line at the top
async fn run_loop(url: String, tx: SyncSender<Msg>) {
    let client = reqwest::Client::new(); // built once, reused across reconnects
    let mut cursor: i64 = 0;
    // ... rest unchanged ...
    match connect_and_stream(&client, &url, cursor, &tx).await {
```

```rust
// connect_and_stream: replace client construction with the parameter
async fn connect_and_stream(
    client: &reqwest::Client,
    url: &str,
    cursor: i64,
    tx: &SyncSender<Msg>,
) -> Result<i64> {
    let response = client                // was: let client = reqwest::Client::new();
        .get(format!("{url}/events/stream"))
        ...
```

- [x] Move `let client = reqwest::Client::new();` to the top of `run_loop`.
- [x] Add `client: &reqwest::Client` as the first parameter of
      `connect_and_stream`.
- [x] Update the two call sites of `connect_and_stream` (one in `run_loop`).
- [x] `cargo test -p curator` green.

---

## Step 3 — Fix `data:` multi-line join per SSE spec (H2)

**File:** `crates/curator/src/sse.rs`

The SSE spec requires that when multiple `data:` lines appear in one event,
their values are joined with a U+000A (LF) character. The current code calls
`push_str(val)` without a separator, merging lines without a delimiter.

Acuity currently sends single-line JSON so no existing test fails, but this
is a real protocol deviation that will bite if the server ever emits
pretty-printed or multi-line data payloads.

**Change in `LineBuffer::feed()`:**

```rust
// Before (sse.rs:124-126):
} else if let Some(rest) = line.strip_prefix("data:") {
    let val = rest.strip_prefix(' ').unwrap_or(rest);
    self.pending_data.push_str(val);
}

// After:
} else if let Some(rest) = line.strip_prefix("data:") {
    let val = rest.strip_prefix(' ').unwrap_or(rest);
    if !self.pending_data.is_empty() {
        self.pending_data.push('\n'); // SSE spec: join multiple data: lines with LF
    }
    self.pending_data.push_str(val);
}
```

This is a no-op for single-line events (the common case): `pending_data` is
empty when the first `data:` line arrives, so no LF is prepended. Existing
tests are unaffected. `serde_json` is tolerant of interior whitespace in JSON,
so multi-line JSON objects joined with LF parse correctly.

**New test** to add in `sse.rs` test block:

```rust
#[test]
fn data_multiline_lines_joined_with_newline() {
    // Feed an event whose JSON is split across two data: lines.
    // The join must produce valid parseable JSON.
    let mut lb = LineBuffer::new(0);
    // Build a JSON string split at a valid token boundary.
    let json = record_json(1, "s1");
    let mid = json.len() / 2;
    let (part1, part2) = json.split_at(mid);
    let frame = format!("id: 1\ndata: {part1}\ndata: {part2}\n\n");
    // With the LF fix, pending_data = "{part1}\n{part2}" which is valid JSON.
    let records = lb.feed(frame.as_bytes());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].seq, 1);
}
```

Note: `serde_json` parses `"{part1}\n{part2}"` correctly as long as the split
falls on a whitespace-tolerant boundary. The split at `json.len() / 2` lands
inside a string value in the JSON produced by `record_json`, which means the
resulting JSON is actually not valid (you can't inject a newline into a JSON
string value without escaping it). The test must be written with a split that
lands on a token boundary. Use a raw frame instead:

```rust
#[test]
fn data_multiline_lines_joined_with_newline() {
    // An event whose data: is split across two lines at a JSON key boundary.
    // The LF between them must not corrupt parsing.
    let mut lb = LineBuffer::new(0);
    let line1 = r#"{"seq":7,"received_at":"2026-01-01T00:00:07Z","event_type":"session_idle","session_id":"s1","turn_id":null,"#;
    let line2 = r#""payload":"{}"}"#;
    let frame = format!("id: 7\ndata: {line1}\ndata: {line2}\n\n");
    let records = lb.feed(frame.as_bytes());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].seq, 7);
    assert_eq!(lb.cursor(), 7);
}
```

- [x] Apply the `if !self.pending_data.is_empty()` guard + `push('\n')` in
      `LineBuffer::feed`.
- [x] Add the `data_multiline_lines_joined_with_newline` test.
- [x] Verify all 28 existing tests still pass: `cargo test -p curator`.

---

## Step 4 — Use `try_send` for SSE events to protect input liveness (C1)

**File:** `crates/curator/src/sse.rs`

The critical issue: on first connect (or after a reconnect following
significant server-side accumulation), the server may replay a large event
burst. If the burst exceeds 4096 channel slots before the main loop drains
them, the SSE thread blocks on `tx.send()`. Since the input thread shares the
same channel, pressing `q` also blocks — the quit signal cannot be enqueued
and the app appears frozen.

Note: once the `?limit_history=N` Slice 2 ships, the cold-start replay no
longer flows through this channel. The `try_send` fix remains correct as
protection against reconnect bursts and live overload; it is not removed.

Fix: use `try_send` for `Msg::Sse` records and silently drop events when the
channel is full. Live telemetry is inherently lossy and the ring buffer already
caps at `EVENT_CAP = 2000`. Only `Msg::Sse` records need this treatment — the
rare `SseStatus` messages remain blocking `send()` calls (they are low-frequency
and critical for UI state correctness).

**Change in `connect_and_stream`:**

```rust
// Add import at top of sse.rs:
use std::sync::mpsc::TrySendError;

// In connect_and_stream, replace (sse.rs:220-224):
for record in lb.feed(&chunk) {
    if tx.send(Msg::Sse(record)).is_err() {
        // Receiver dropped — main thread exited cleanly.
        return Ok(lb.cursor());
    }
}

// With:
for record in lb.feed(&chunk) {
    match tx.try_send(Msg::Sse(record)) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            // Channel is saturated (reconnect burst or live overload).
            // Drop this event — telemetry is inherently lossy and the ring
            // buffer (EVENT_CAP) already bounds what the UI shows.
            // Input liveness (including Quit) is never blocked.
        }
        Err(TrySendError::Disconnected(_)) => {
            // Receiver dropped — main thread exited cleanly.
            return Ok(lb.cursor());
        }
    }
}
```

**Why this is safe:**

- Dropped events are telemetry — the ring buffer already bounds the UI to
  `EVENT_CAP = 2000` events regardless. The cursor still advances correctly:
  `lb.cursor()` tracks the last *parsed* seq, not the last *enqueued* seq.
  On reconnect, `Last-Event-ID` resumes from after the dropped events. This
  is the correct lossiness model for live telemetry.
- `SseStatus` messages continue to use blocking `send()` — they are rare (one
  per reconnect attempt) and their delivery is critical for UI correctness.
- The input thread is unaffected: it only sends `Msg::Input` and `Msg::Redraw`,
  both of which remain blocking `send()`. Since user input is slow relative to
  the channel capacity (a human can produce at most ~10 keystrokes/sec), the
  input thread will never fill the channel.

**Caveat to document in the code comment:** dropped SSE records advance the
cursor but are not rendered. If the backlog exceeds `EVENT_CAP` events, the
oldest events that overflow the ring buffer are invisible to the user — this is
the same behaviour as the ring buffer eviction that already exists.

- [x] Add `use std::sync::mpsc::TrySendError;` import to `sse.rs`.
- [x] Replace `tx.send(Msg::Sse(record)).is_err()` with the `try_send` match
      shown above.
- [x] `cargo test -p curator` green.

---

## Step 5 — Clarify run loop shutdown semantics (H1)

**File:** `crates/curator/src/main.rs`

The `Err(_) => break` arm at `main.rs:112` is unreachable dead code. The
original `tx` remains in scope inside `main()` while `run()` executes, so
`recv()` can never return `Err(RecvError)` during a live run. If a future
refactor moves `tx` ownership into `run()`, the `break` silently falls through
to `Ok(())` at the bottom of the loop, skipping a final draw.

**Fix:** Drop `tx` explicitly in `main()` before calling `run()`, then change
the `Err` arm to `return Ok(())` with a clear comment.

In `main.rs`, after the thread spawns and the `tx.send(Disabled)` path
(currently lines 70-79), add `drop(tx);`:

```rust
// Always spawn the input thread.
input::spawn(tx.clone());

// Spawn the SSE thread only when an acuity URL is configured; otherwise
// notify the app immediately that SSE is disabled.
if let Some(url) = cli.acuity_url {
    sse::spawn(url, tx.clone());
} else {
    let _ = tx.send(Msg::SseStatus(SseStatus::Disabled));
}

// Drop the original tx so that when all thread senders have exited,
// rx.recv() returns Err and run() can exit cleanly via that path.
drop(tx);
```

Then in `run()`, change the `Err` arm:

```rust
// Before:
Err(_) => break, // all senders dropped — shouldn't happen normally

// After:
Err(_) => {
    // All senders dropped — both leaf threads have exited.
    // This is the clean-shutdown path when threads terminate before Quit.
    return Ok(());
}
```

Remove the now-dead trailing `Ok(())` at `main.rs:120` (the one after the
loop), or leave it as the unreachable fallthrough — with the `return` above,
the loop body's `break` arm is removed so the `Ok(())` at the end of the
function is reached only if somehow the loop exits without returning, which
is now impossible. To make the function compile without the dead code, the
loop can be `loop { ... }` with all exits via `return`, removing the trailing
`Ok(())`.

Revised `run()` body:

```rust
fn run(
    terminal: &mut tui::Tui,
    app: &mut App,
    rx: Receiver<Msg>,
    root: &Path,
    branch: &str,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        match rx.recv() {
            Ok(msg) => {
                if process_msg(msg, app, root, branch)? == LoopControl::Quit {
                    return Ok(());
                }
            }
            Err(_) => {
                // All senders dropped — both leaf threads have exited.
                return Ok(());
            }
        }
        while let Ok(msg) = rx.try_recv() {
            if process_msg(msg, app, root, branch)? == LoopControl::Quit {
                return Ok(());
            }
        }
    }
}
```

With all exits as explicit `return`, the function has no fall-through path and
no trailing `Ok(())` is needed. Rust will accept an infinite `loop` without a
final expression if all paths return.

- [x] Add `drop(tx);` in `main()` after all thread spawns.
- [x] Replace `Err(_) => break` with `Err(_) => { return Ok(()); }` plus
      explanatory comment.
- [x] Remove the trailing `Ok(())` from `run()` (compiler will confirm all
      paths return).
- [x] `cargo clippy --workspace -- -D warnings` clean.
- [x] `cargo test --workspace` green (all 28+ tests pass).

---

## Step 6 — Final verification

- [x] `cargo clippy --workspace -- -D warnings` — zero warnings.
- [x] `cargo test --workspace` — all tests green.
- [x] Confirm `cargo build -p curator` succeeds.
- [x] Review the diff of this fix slice against HEAD and confirm only the
      five targeted files/locations are changed.
