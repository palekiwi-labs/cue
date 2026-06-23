---
status: open
priority: low
---
# Deferred acuity SSE/quality follow-ups

Captured during the opus-review fix work on `feat/acuity-api-mvp`
(task: `master/task/1782211100-a8d0fb9/acuity-api-review-fixes.md`).
These were intentionally deferred (agreed with user) to keep the
review-fix PR focused.

Supplemented post-merge with an Opus performance/ordering consultation
(see master/spec/log.md entry "Phase 5 merged").

ref: `.cue/feat-acuity-api-mvp/trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md`

---

## SSE redesign (Major #3, full) — Phase 7

Replace the `sleep(500ms)` in the SSE poll loop with a
`tokio::sync::broadcast` notification channel so the stream wakes
immediately when a new row is inserted.

**Ordering recommendation (Opus):** Defer until after Phase 6
dogfooding. The SSE wire contract (`text/event-stream`,
`Last-Event-ID` resume) is unchanged by this redesign, so Phase 6
does not need rework when it lands. Gate on empirical signal: if
~500 ms latency is not annoying in the live curator TUI, the poll
loop is fine to keep indefinitely for a single-consumer dev tool.

### Performance analysis (Opus)

The only performance dimension that matters at this scale is
**delivery latency** (~500 ms average → ~0 ms). Everything else is
negligible for a single-consumer, local-only tool:

- **Idle CPU**: 2 wakeups/sec each running a zero-row PK-scan SELECT
  — microseconds of work, irrelevant.
- **SQLite contention**: exists in both approaches (DELETE journal
  mode, no busy_timeout in `db::connect`). Push slightly *increases*
  the chance of a SELECT/commit race because wakes are immediate. This
  is an orthogonal concern, not a reason to prefer poll.
- **Backpressure during bursts**: the broadcast redesign does **NOT**
  remove the bounded drain loop (`SSE_MAX_DRAIN_PAGES`) or the
  catch-up machinery. Only the `sleep(500ms)` is replaced by
  `recv()`. The drain loop must stay to protect keepalive during
  bursts and to handle `Last-Event-ID` reconnect. The original todo
  claim that push "removes the catch-up/bounded-drain machinery
  entirely" is incorrect.

### Implementation sketch (Phase 7)

**`AppState`** (`main.rs:24-31`): add
`events_tx: tokio::sync::broadcast::Sender<()>`. Construct in `main`
with `broadcast::channel(16).0`; `Sender` is `Clone` so `AppState`
stays `Clone`.

**`handle_event`** (`main.rs:335-348`): after a successful
`insert_event`, add `let _ = state.events_tx.send(());` — ignore the
zero-subscriber `Err` (no curator attached = fine). Must be strictly
*after* the commit so the wake never races ahead of durability.

**`sse_handler`** (`main.rs:132-193`): `let mut rx =
state.events_tx.subscribe();` before the loop. Keep the bounded drain
exactly as-is. Replace `tokio::time::sleep(500ms)` (`main.rs:188`)
with `tokio::select!` over `rx.recv()` and a keepalive timer.
Handle `rx.recv()` returning `Ok(())` **or** `Err(Lagged(_))`
identically: fall through and drain from `seq`. `Err(Closed)` ends
the stream.

**Critical footgun — `RecvError::Lagged`**: `broadcast` has a
fixed-capacity ring. If a burst sends more `()` signals than the
channel capacity (16) before the consumer drains, `recv()` returns
`Lagged`. With a `()` payload this is harmless — it just means "drain
again" — but **must be handled explicitly** or the stream errors out.

**Tests to add/change** (`tests.rs`):
1. Live-delivery latency test: open the stream, POST an event, assert
   the new frame arrives well under 500 ms. This is the regression
   guard for the latency win.
2. `Lagged` recovery test: burst more than 16 events before the
   consumer reads, assert the stream recovers and delivers all rows
   without erroring.
3. Disconnect/no-leak test (see section below — do this first).

The existing first-frame and `Last-Event-ID` resume tests
(`tests.rs:353-396`) should pass unchanged, proving the wire contract
is stable.

---

## SSE disconnect / no-leak test (Major #4) — do before redesign

Add a test that drops the SSE response body mid-stream and asserts the
poll loop terminates (no leaked task, no panic). Currently the
disconnect path relies entirely on axum dropping the stream and is
unverified.

**Priority note (Opus):** This is the higher-value target before
Phase 7. It covers a correctness gap regardless of whether the loop
strategy is poll or push, and is independent of the broadcast
redesign. If SSE effort is to be spent before Phase 7, do this first.

---

## Cosmetic nits — low priority

- Pre-existing `expect("serde_json validated UTF-8")` at `main.rs`
  (`String::from_utf8(body.to_vec())` in `handle_event`). Sound but
  panic-free `from_utf8_lossy` or retaining the validated `&str`
  path is more robust.
- Redundant `after` clamp: handler clamps `after >= 0` and the DB
  clamps `limit`; `seq > after` with a negative `after` is already
  harmless in SQL.
- 80-col soft-guideline violations in `tests.rs` (rustfmt preserves
  them; project cue style prefers 80).
