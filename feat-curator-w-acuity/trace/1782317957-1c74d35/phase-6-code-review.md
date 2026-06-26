# Code Review — Phase 6 Curator SSE Live Views

Fused output from two independent reviewers: `diff-reviewer-gemini-3-flash`
and `consultant-opus`. Reviewed against the full diff of
`feat/curator-w-acuity` (merge base `be00e7e`) and the implementing agent's
self-assessment trace (`trace/1782317957-1c74d35/slice-5-8-implementation-trace.md`).

---

## CRITICAL

### C1 — Channel backpressure can starve the `q` key

**Location:** `crates/curator/src/main.rs:68`,
`crates/curator/src/sse.rs:220-225`
**Reviewers:** Opus (Flash missed this)

The input thread and SSE thread share the same bounded
`sync_channel(4096)`. `SyncSender::send()` **blocks** when full. On the
very first connect with `Last-Event-ID: 0`, the server replays the entire
backlog (paginated, no inter-page delay). If the backlog exceeds 4096
events, the SSE thread blocks indefinitely. If the input thread then tries
to `send()` while the channel is saturated (e.g. user pressing `q`), the
quit signal cannot be enqueued and the application appears frozen — exit is
only possible via signal.

**Recommendation:** Use `try_send` on the SSE path and drop (or coalesce)
events when the channel is full — live telemetry is inherently lossy and the
ring buffer is already capped at `EVENT_CAP = 2000`. Alternatively, give SSE
its own separate channel so input (especially `Quit`) is never starved by
event volume.

---

## HIGH

### H1 — `Err(_)` arm of `recv()` is dead code and masks a latent footgun

**Location:** `crates/curator/src/main.rs:109-112`
**Reviewers:** Opus only

The original `tx` remains in scope in `main()` while `run()` executes, so
`recv()` can never return `Err`. The `Err(_) => break` arm is unreachable
dead code — not "unlikely" as the comment says. If a future refactor moves
`tx` ownership into `run()`, the `break` falls through to an implicit
`Ok(())` at the bottom, silently skipping a final draw and returning without
any diagnostic.

**Recommendation:** Either add `unreachable!()` with a comment explaining
the ownership invariant, or intentionally invert the ownership model so this
path becomes the deliberate clean-shutdown route.

### H2 — SSE `data:` multi-line join missing newline per spec

**Location:** `crates/curator/src/sse.rs:126-128`
**Reviewers:** Flash only

The SSE spec requires each `data:` field value to be appended with a LF
character before concatenation. The current code calls `push_str(val)`
without a trailing `\n`. If the server ever sends multi-line JSON across
multiple `data:` fields, the lines will concatenate without a separator,
producing invalid JSON. The server currently sends single-line data, so this
is latent but a real protocol deviation.

**Recommendation:**
```rust
self.pending_data.push_str(val);
self.pending_data.push('\n'); // SSE spec: each data: value ends with LF
```

---

## MEDIUM

### M1 — `reqwest::Client` rebuilt on every reconnect

**Location:** `crates/curator/src/sse.rs:199`
**Reviewers:** Both agreed

A new `Client` (with fresh TLS stack and cert store load) is created inside
`connect_and_stream` on every reconnect attempt. `reqwest::Client` is
designed to be reused for connection pooling. At the 5s backoff cap, a
flapping server triggers a new TLS handshake every 5s.

**Recommendation:** Build the `Client` once at the top of `run_loop` and
pass `&Client` into `connect_and_stream`.

### M2 — `received_at > entry.last_seen` relies on lexicographic string comparison

**Location:** `crates/curator/src/app.rs:195`
**Reviewers:** Opus only

`received_at` is a `String`. The comparison is byte-lexicographic, which is
correct only if the server always emits fixed-width, zero-padded, uniform-
precision UTC ISO-8601. Mixed sub-second precision (`...00Z` vs `...00.5Z`)
inverts the ordering lexicographically. Currently safe because both ends are
in this repo, but the invariant is undocumented and fragile.

**Recommendation:** Compare `seq` (a monotone integer) instead of
`received_at` for "is newer", or document the strict format invariant at
the comparison site with a test that asserts the server format.

### M3 — `eprintln!` inside SSE thread corrupts TUI output

**Location:** `crates/curator/src/sse.rs:109, 159, 186`
**Reviewers:** Flash only

Writing to stderr while ratatui owns the terminal overwrites the UI with raw
text, causing visual corruption that persists until the next full redraw.

**Recommendation:** Route errors as a new `Msg::SseError(String)` variant to
the main loop and surface them in the Diagnostics view or a status bar, or
log to a file.

### M4 — `process_msg` untestable as a free function in `main.rs`

**Location:** `crates/curator/src/main.rs:123-146`
**Reviewers:** Both agreed

The message→state transition logic cannot be unit-tested without constructing
a full `Receiver`/terminal harness. Moving it to `App::handle_msg` (or
similar) would match the testing discipline already applied to `push_event`,
`event_summary`, and `LineBuffer`.

---

## LOW

### L1 — `SessionSummary.session_id` `#[allow(dead_code)]` comment is factually wrong

**Location:** `crates/curator/src/app.rs:77-79`
**Reviewers:** Both agreed; Opus verified in tests

No Slice 8 test reads `.session_id`. The comment claiming "used in Slice 8
unit tests" is inaccurate. The HashMap key is already the session id;
storing it again inside the struct is genuine redundancy.

**Recommendation:** Remove the field and the `#[allow(dead_code)]` attribute.

### L2 — `record_json` test helper hand-rolls JSON via `format!`

**Location:** `crates/curator/src/sse.rs:243-247`
**Reviewers:** Both agreed

The helper will silently produce invalid JSON if `EventRecord`'s field names
or types change, causing confusing test failures. The `app.rs` and `ui.rs`
tests correctly use `serde_json::to_string(&event)`.

**Recommendation:** Build an `EventRecord` struct and `serde_json::to_string`
it, consistent with the other test modules.

### L3 — Flapping server hits 500ms reconnect loop indefinitely with no jitter

**Location:** `crates/curator/src/sse.rs:180`
**Reviewers:** Opus only

A server that accepts the connection (HTTP 200) then immediately closes the
stream causes `connect_and_stream` to return `Ok(lb.cursor())`. This resets
`backoff_ms` to 500ms unconditionally, treating an immediate-close as a
healthy long-lived stream. A perpetually flapping server is polled every
500ms forever with no backoff escalation.

**Recommendation:** Distinguish "stream stayed open for a meaningful duration
and produced events" from "immediate close after 200". Only reset backoff
after a minimum healthy duration. Add small random jitter.

### L4 — Tokio runtime-build failure shows "reconnecting" forever (misleading)

**Location:** `crates/curator/src/sse.rs:158-161`
**Reviewers:** Opus only

If `tokio::runtime::Builder::build()` fails, the SSE thread sends
`Reconnecting { attempt: 1 }` and exits permanently. The UI displays
"reconnecting (attempt 1)" forever — a lie; the thread is permanently dead.

**Recommendation:** Add a terminal `SseStatus::Failed(String)` variant for
unrecoverable thread death, distinct from transient `Reconnecting`.

---

## Verified Non-Issues

| Concern | Verdict |
|---|---|
| `attempt` reset-to-0 then `+= 1` producing `attempt: 1` | Correct — first reconnect after healthy session correctly shows attempt 1 |
| `Last-Event-ID: 0` skipping the first event | Not a bug — server uses `WHERE seq > after` (strict `>`); `0` returns all events from seq 1 |
| `sel_activity` visual mapping correctness | Correct — `items.len()` is captured before the event row is pushed, which is the row index that row will occupy |
| `scroll_down_activity` stopping at `events.len()-1` | Acceptable UX — last event (not header) is always reachable; headers are non-interactive |
| Diagnostics selection bound | Tight and correct — `diagnostics_len()` and the render filter both use the `tool_call_*` predicate |
| `clap` env feature | Fixed in `Cargo.toml:9` (confirmed) |

---

## Merge Recommendation

**Block on C1** (backpressure starving quit). An application that ignores
`q` under a large historical backlog is a hard usability failure.

**Strongly recommended before merge:** H1 (dead `Err` arm), H2 (SSE newline
per spec), M3 (eprintln TUI corruption), M1 (client reuse).

**Safe to defer post-merge:** M2, L1, L2, L3, L4.
