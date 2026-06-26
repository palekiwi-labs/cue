---
status: complete
---
# Phase 6 — Curator Live Half

## Foreword

This plan wires `curator` to `acuity`'s SSE stream, adding two new live
views (Activity Feed and Diagnostics) alongside the existing Phase 2 kanban.

It addresses Phase 6 of the cue ecosystem roadmap:
`task/1781965432-d2f3251/curator-live-half.md`

**Architecture selected (Opus-reviewed):** Unified-channel variant of Option A.
Two leaf threads — one for crossterm input, one for SSE — both posting into a
single `std::sync::mpsc` channel as a `Msg` enum. The main loop blocks on
`rx.recv()`, drains all pending messages, then draws once. No async runtime in
the main thread. Zero borrow/cancellation hazards.

**Critical correctness findings (from Opus review):**

1. Session summaries must be maintained **incrementally** in a `HashMap`
   updated in `push_event()` *before* ring-buffer eviction — fold-on-render
   is silently wrong once old `SessionIdle` events are evicted.
2. SSE parsing must **line-buffer across chunk boundaries** and skip `:` keep-
   alive comment lines or reconnect storms occur every 15s.
3. SSE connection state must be surfaced via `Msg::SseStatus` so live views
   show degraded state rather than silently freezing.

**Prerequisites:**
- `feat/curator-with-acuity` branch checked out
- `acuity` server running locally (for manual testing of Views 2 and 3)
- All existing curator tests green before any changes

---

## Slice 1 — Dependencies and `Msg` enum

**Goal:** Add new crate deps and define the unified message type.

- [x] Add to `crates/curator/Cargo.toml`:
  ```toml
  acuity-api  = { path = "../acuity-api" }
  serde_json  = "1"
  tokio       = { version = "1", features = ["rt", "time", "net", "macros"] }
  reqwest     = { version = "0.12", default-features = false,
                  features = ["rustls-tls", "stream"] }
  ```
  Note: `tokio`, `reqwest`, `futures-core`, `tokio-util` are already in the
  workspace lockfile via `acuity`; this adds zero new crates to the build graph.

- [x] Create `crates/curator/src/msg.rs`:
  - Define `Msg` enum:
    ```rust
    pub enum Msg {
        Input(Action),
        Redraw,                       // terminal resize
        Sse(EventRecord),
        SseStatus(SseStatus),
    }

    pub enum SseStatus {
        Connected,
        Reconnecting { attempt: u32 },
        Disabled,                     // no acuity-url configured
    }
    ```
  - `EventRecord` imported from `acuity_api`; `Action` from `event.rs`.
  - Both `EventRecord` and `SseStatus` must be `Send`.

- [x] Verify `cargo build -p curator` compiles with new deps.

---

## Slice 2 — Input thread

**Goal:** Move crossterm input from the main loop poll into a dedicated thread
posting `Msg::Input` and `Msg::Redraw` to the unified channel.

- [x] Create `crates/curator/src/input.rs`:
  - `pub fn spawn(tx: SyncSender<Msg>)` — spawns a `std::thread`
  - Thread body: loop on `crossterm::event::read()` (blocking, no timeout)
  - Match on `Event::Key` → map to `Action` → send `Msg::Input(action)`
  - Match on `Event::Resize` → send `Msg::Redraw`
  - On `tx.send()` error: break (receiver dropped = main thread quit)
  - Key mapping is identical to current `event.rs` — add `1/2/3` for view
    switching: `Char('1') → Action::SwitchView(View::Kanban)`, etc.
  - Add `Char('r') → Action::Refresh`

- [x] Update `crates/curator/src/event.rs`:
  - Extend `Action` enum:
    ```rust
    SwitchView(View),   // View enum defined in app.rs, imported here
    Refresh,
    ```
  - Keep the existing `next_action()` function intact for now; it will be
    removed in the final cleanup after `input.rs` takes over.

- [x] All existing tests must still pass.

---

## Slice 3 — SSE thread

**Goal:** A self-contained thread that opens `GET /events/stream`, parses SSE
events with a proper line-buffer, and reconnects with cursor on any error.

- [x] Create `crates/curator/src/sse.rs`:

  **`pub fn spawn(url: String, tx: SyncSender<Msg>)`** — entry point. Spawns a
  `std::thread`. Inside:

  1. Build a `tokio::runtime::Builder::new_current_thread().enable_all().build()`
     runtime. On error: send `Msg::SseStatus(SseStatus::Reconnecting)`, log, and
     return — *never* panic.

  2. `rt.block_on(run_loop(url, tx))`

  **`async fn run_loop(url, tx)`**:
  - Track `cursor: i64 = 0` and `backoff_ms: u64 = 500`
  - Loop:
    - Send `Msg::SseStatus(Reconnecting { attempt })` before each connect
    - Call `connect_and_stream(&url, cursor, &tx).await`
    - On `Ok(last_seq)`: update `cursor = last_seq`, reset `backoff_ms = 500`,
      send `Msg::SseStatus(Connected)`
    - On `Err(_)`: log the error, sleep `backoff_ms`, cap at 5000ms
      (`backoff_ms = (backoff_ms * 2).min(5000)`)

  **`async fn connect_and_stream(url, cursor, tx) -> Result<i64>`**:
  - Build `reqwest::Client` once (can be passed in or built per call for
    simplicity)
  - Send `GET {url}/events/stream` with header `Last-Event-ID: {cursor}`
  - Check `response.status().is_success()`; error if not
  - Create a `LineBuffer` (see below), then consume `response.bytes_stream()`:
    `while let Some(chunk) = stream.next().await { for rec in lb.feed(&chunk?) { ... } }`
  - Stream exhausted (server closed): return `Ok(lb.cursor())`

  **`LineBuffer` struct (extracted for testability — Opus recommendation):**

  The entire SSE parse logic lives in a synchronous, pure struct so that Slice 8
  can unit-test every correctness case without a server:

  ```rust
  struct LineBuffer {
      buf: Vec<u8>,
      pending_id: Option<i64>,
      pending_data: String,
      cursor: i64,       // last successfully parsed id
  }

  impl LineBuffer {
      fn new(initial_cursor: i64) -> Self { ... }
      /// Feed one raw chunk; return any EventRecords completed by this chunk.
      fn feed(&mut self, chunk: &[u8]) -> Vec<EventRecord> { ... }
      fn cursor(&self) -> i64 { self.cursor }
  }
  ```

  `feed()` internal logic:
  - Append `chunk` to `self.buf`
  - Split on `\n`, keep the partial trailing bytes in `self.buf`
  - Per complete line:
    - Skip lines starting with `:` (keep-alive comments — skipping prevents
      reconnect storms every 15 s)
    - Parse `id: <value>` → store as `pending_id`
    - Parse `data: <value>` → store as `pending_data` (strip exactly one
      leading space per SSE spec)
    - On blank line (event terminator):
      - If `pending_data` is empty: clear accumulators and continue
      - `serde_json::from_str::<EventRecord>(&pending_data)` — on error: log
        and continue (don't reconnect for malformed rows)
      - Update `self.cursor` from `pending_id`
      - Push completed record to output vec
      - Clear `pending_id`, `pending_data`
  - Return completed records

  **`next_backoff(current_ms: u64) -> u64` (pure helper):**
  ```rust
  pub(crate) fn next_backoff(current_ms: u64) -> u64 {
      (current_ms * 2).min(5000)
  }
  ```
  Extracted so Slice 8 can trivially test the 5000 ms cap.

- [x] Unit tests for `sse.rs` live in Slice 8. Parser logic is pure/sync
  so no live server is required — see Slice 8 tier 2.

---

## Slice 4 — App state extension

**Goal:** Add view state, event ring buffer, and incremental session map to
`App`. The existing kanban fields are untouched.

- [x] Add to `crates/curator/src/app.rs`:

  **New enums:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum View { Kanban, Activity, Diagnostics }

  #[derive(Debug, Clone)]
  pub enum AcuityStatus {
      Disabled,
      Connected,
      Reconnecting { attempt: u32 },
  }
  ```

  **`SessionSummary` struct:**
  ```rust
  pub struct SessionSummary {
      pub session_id: String,
      pub project_dir: String,
      pub session_title: Option<String>,
      pub first_seen: String,   // ISO-8601, from earliest received_at
      pub last_seen: String,    // ISO-8601, from latest received_at
      pub input_tokens: u64,
      pub output_tokens: u64,
      pub error_count: u32,
  }
  ```

  **New `App` fields** (add to existing struct):
  ```rust
  pub active_view: View,
  pub acuity_status: AcuityStatus,

  // Ring buffer — oldest at front, newest at back
  pub events: VecDeque<EventRecord>,
  // Incremental session map — survives ring-buffer eviction
  pub sessions: HashMap<String, SessionSummary>,

  pub sel_activity: usize,
  pub sel_diagnostics: usize,
  ```

  **`App::push_event(&mut self, record: EventRecord)`:**
  1. Update `self.sessions` from the incoming record *before* eviction:
     - Always: update `last_seen` if `record.received_at > current.last_seen`
     - `session_idle`: set `project_dir`, `session_title`, set `first_seen`
       if entry is new
     - `agent_turn_completed`: deserialize `payload`, add
       `input_tokens`/`output_tokens`
     - `tool_call_completed`: deserialize `payload`, increment `error_count`
       if `is_error == true`
     - Upsert into `self.sessions` keyed by `record.session_id`
  2. Evict: `if self.events.len() == 2000 { self.events.pop_front(); }`
  3. `self.events.push_back(record)`

  **Navigation methods** (mirroring existing `scroll_down`/`scroll_up`):
  - `scroll_down_activity()` / `scroll_up_activity()` — bounded by
    `self.events.len()`
  - `scroll_down_diagnostics()` / `scroll_up_diagnostics()` — bounded by
    filtered tool-call count (or compute on the fly)

- [x] Add `use std::collections::{HashMap, VecDeque}` and
  `use acuity_api::{AcuityEvent, EventRecord}` to `app.rs`.

- [x] Write `push_event` unit tests **red-green alongside the implementation**
  (TDD — write the failing test first, then the code):
  - Eviction at cap: assert ring buffer stays ≤ 2000
  - Token accumulation: tokens sum across multiple `AgentTurnCompleted`
  - Project attribution survives eviction: push 2001 events (first is
    `SessionIdle`), assert `sessions` map still has `project_dir`
  - Error counting: `ToolCallCompleted { is_error: true }` increments count

  These tests are expanded and joined by `LineBuffer` and render-helper tests
  in Slice 8.

---

## Slice 5 — Unified run loop and main.rs wiring

**Goal:** Replace the existing poll-based run loop with a unified `rx.recv()`
loop. Wire the SSE thread and input thread.

- [x] Update `crates/curator/src/main.rs`:

  **CLI additions:**
  ```rust
  /// Base URL of the acuity server (e.g. http://localhost:3030).
  /// Falls back to $ACUITY_URL if not set. Omit to run in kanban-only mode.
  #[arg(long, env = "ACUITY_URL", value_name = "URL")]
  acuity_url: Option<String>,
  ```
  Note: `clap`'s `env` attribute reads the env var automatically.

  **Wiring in `main()`:**
  ```rust
  let (tx, rx) = std::sync::mpsc::sync_channel::<Msg>(4096);

  // Always spawn the input thread
  input::spawn(tx.clone());

  // Spawn SSE thread only if acuity-url is configured
  if let Some(url) = cli.acuity_url {
      sse::spawn(url, tx.clone());
  } else {
      // Inform App of disabled state (sent before run loop starts)
      let _ = tx.send(Msg::SseStatus(SseStatus::Disabled));
  }

  let mut app = App::new(tasks);
  let run_result = run(&mut terminal, &mut app, rx);
  ```

  **New `run()` signature:**
  ```rust
  fn run(
      terminal: &mut tui::Tui,
      app: &mut App,
      rx: Receiver<Msg>,
  ) -> Result<()>
  ```

  **New `run()` body:**
  ```rust
  loop {
      terminal.draw(|frame| ui::render(frame, app))?;

      // Block until at least one message arrives, then drain all pending.
      // This gives instant keyboard response and batches SSE bursts.
      match rx.recv() {
          Ok(msg) => process_msg(msg, app),
          Err(_) => break,   // all senders dropped (shouldn't happen normally)
      }
      while let Ok(msg) = rx.try_recv() {
          if process_msg(msg, app) == LoopControl::Quit { return Ok(()); }
      }
  }
  Ok(())
  ```

  **`process_msg(msg, app) -> LoopControl`:**
  ```rust
  match msg {
      Msg::Input(Action::Quit) => return LoopControl::Quit,
      Msg::Input(Action::SwitchView(v)) => app.active_view = v,
      Msg::Input(Action::Refresh) => reload_tasks(app, &root, &branch)?,
      Msg::Input(Action::Down) => app.scroll_down(),
      // ... etc per active_view
      Msg::Redraw => {}  // just triggers a redraw on the next loop
      Msg::Sse(record) => app.push_event(record),
      Msg::SseStatus(s) => app.acuity_status = s.into(),
  }
  ```

  Navigation dispatch (Down/Up) checks `app.active_view` and calls the
  appropriate method (`scroll_down` / `scroll_down_activity` / etc.).

- [x] Remove the old `next_action()` call from the run loop (it is now dead
  code; `event.rs`'s `next_action()` function can be removed or left as
  dead code until cleanup).

- [x] Reload helper:
  ```rust
  fn reload_tasks(app: &mut App, root: &Path, branch: &str) -> Result<()> {
      let tasks = read_artifacts(root, branch, "task")?;
      app.reload_kanban(tasks);
      Ok(())
  }
  ```
  Add `App::reload_kanban(&mut self, tasks: Vec<ArtifactMeta>)` that
  re-classifies tasks into the three column vectors and resets selection
  indices.

---

## Slice 6 — Rendering: view switching, Activity Feed, Diagnostics

**Goal:** Implement the three-view UI. Kanban stays unchanged; two new
render functions added.

- [x] Update `crates/curator/src/ui.rs`:

  **Top-level `render()`:** dispatch on `app.active_view`:
  ```rust
  pub fn render(frame: &mut Frame, app: &App) {
      match app.active_view {
          View::Kanban      => render_kanban(frame, app),
          View::Activity    => render_activity(frame, app),
          View::Diagnostics => render_diagnostics(frame, app),
      }
  }
  ```
  Rename existing `render()` to `render_kanban()`.

  **Shared layout helper:** extract `layout_with_help(area) -> (Rect, Rect)`
  (board area + 1-line help bar) since all three views use it.

  **`render_activity(frame, app, area)`:**
  - Layout: main list area + 1-line help/status bar
  - Build `ListItem`s from `app.events.iter().rev()`:
    - When `session_id` changes between adjacent rows, insert a styled
      header line: `"── <project_dir_basename> (<short_session_id>) ──"`
      derived from `app.sessions.get(&session_id)`
    - Per event row: `" {received_at_short}  {event_type:<24}  {summary}"`
      where `summary` is a one-liner derived from the payload (see below)
    - `received_at_short`: last 19 chars of ISO-8601 (`T` to `Z` slice)
  - Payload summary helpers (match on `record.event_type` to avoid
    deserializing every row — only deserialize the rows that need a summary,
    or carry summary fields computed in `push_event`):
    - `session_idle` → `"idle: {session_title}"`
    - `agent_turn_completed` → `"turn: in={input_tokens} out={output_tokens}"`
    - `tool_call_requested` → `"tool: {tool_name}"`
    - `tool_call_completed` → `"done: {tool_name}"` or
      `"ERR:  {tool_name} — {error_text}"`
  - Selection: `ListState` with `app.sel_activity`
  - Status bar:
    ```
    q quit  1/2/3 views  j/k navigate  |  acuity: connected
    ```
    Colour-code status: green=Connected, yellow=Reconnecting, gray=Disabled

  **`render_diagnostics(frame, app, area)`:**
  - Filter `app.events.iter().rev()` where `e.event_type` starts with
    `"tool_call_"` — **no payload deserialization for filtering**
  - For each visible row: deserialize `payload` → `AcuityEvent` to extract
    `tool_name`, `is_error`, `error_text`
  - Style: `ToolCallRequested` in default colour; `ToolCallCompleted` with
    `is_error: false` in green; `is_error: true` in red with error text
  - Same `ListState`/`sel_diagnostics` selection
  - Same status bar as Activity

  **Kanban help bar update:** add `1/2/3 views` hint.

- [ ] Priority ordering in kanban: update `App::new()` to sort each column
  by priority after classification:
  ```rust
  fn priority_rank(p: Option<&str>) -> u8 {
      match p { Some("critical") => 0, Some("high") => 1,
                Some("low") => 3, _ => 2 }
  }
  open.sort_by_key(|t| priority_rank(t.priority_raw.as_deref()));
  ```

---

## Slice 7 — Cleanup and manual QA

**Goal:** Remove dead code, confirm all automated tests pass, then hand off the
manual smoke-test checklist to the human developer.

- [x] **Cleanup:**
  - Remove dead `next_action()` function from `event.rs` if unused
  - Remove `next_action` import from `main.rs`
  - Confirm `cargo clippy --workspace -- -D warnings` is clean
  - Confirm `cargo test --workspace` is green

- [ ] **Update `docs/curator.md`:** stub note that View 2/3 require
  `--acuity-url` or `$ACUITY_URL`.

- [ ] **Manual smoke test (human-performed — requires live acuity):**

  Build release binary and start acuity, then run:
  ```
  cargo run -p curator -- --acuity-url http://localhost:3030
  ```

  Checklist:
  1. Confirm kanban loads (View 1), tasks sorted by priority
  2. Press `2` → Activity Feed; verify events in reverse-chronological order
  3. Press `3` → Diagnostics; verify only tool-call events, errors in red
  4. Trigger a new agent session; verify live events arrive without pressing `r`
  5. Press `r` on View 1; verify task list reloads from disk
  6. Kill acuity; verify status bar shows "reconnecting" (yellow)
  7. Restart acuity; verify reconnection — **confirm no duplicate events**
     (step 7 specifically targets cursor-resume correctness — the one case
     automated tests cannot fully exercise end-to-end)

  Agent will provide these instructions at the start of Slice 7.

---

## Slice 8 — Automated test coverage

**Goal:** Unit-test the two highest-risk components (SSE parser, App state) and
extract render helpers for testing. No new dependencies; no live server needed.

Per Opus review: tiers 1+2 cover ~90% of the risk. Tier 3 is optional.

### Tier 1 — App state unit tests (highest ROI)

- [x] Expand `push_event` tests from Slice 4 with any gaps:
  - Eviction at cap (≤ 2000)
  - Token accumulation across multiple `agent_turn_completed` events
  - Project attribution (`project_dir`) survives ring-buffer eviction
  - Error count increments on `tool_call_completed { is_error: true }`

- [x] Additional `App` tests:
  - Priority sort: after `App::new()` tasks appear in canonical priority order
    (`critical → high → normal → low`)
  - `reload_kanban`: re-classifies, re-sorts, resets all `sel_*` indices to 0

- [x] `next_backoff` unit test:
  - Doubles from 500 → 1000 → 2000 → 4000 → 5000 (capped)

### Tier 2 — `LineBuffer` parser unit tests (highest risk, zero infrastructure)

These are synchronous `#[test]` functions — no async, no server, no extra deps.

- [x] Normal single-event: feed one complete SSE frame as a single chunk,
  assert one `EventRecord` returned with correct `seq` and `session_id`

- [x] Chunk boundary split: feed a single SSE frame as two chunks (split
  mid-`data:` line), assert the record is not emitted until the second chunk
  completes the event

- [x] Keep-alive skip: feed `:keep-alive\n` between two events, assert only
  two records returned (keep-alive does not corrupt state)

- [x] Leading-space strip: `data: {"seq":1,...}` (one space after colon)
  parses correctly; `data:{"seq":1,...}` (no space) also parses correctly

- [x] Malformed JSON: feed a frame with invalid `data:` JSON, assert zero
  records returned (no panic, no reconnect)

- [x] Multiple events in one chunk: three complete SSE frames in a single
  `feed()` call, assert three records returned in order

- [x] Blank-line no-op: a blank line with empty `pending_data` produces no
  record

- [x] Cursor tracking: after N events, `lb.cursor()` equals the `seq` of the
  last successfully parsed event

### Tier 3 — Thin end-to-end wiring test (optional)

- [ ] (Optional) Spin up a `tokio::net::TcpListener` on `127.0.0.1:0` in a
  `#[tokio::test]`, write a raw HTTP/1.1 SSE response (two frames then close),
  call `sse::spawn(url, tx)`, assert two `Msg::Sse` variants arrive on `rx`
  within a timeout. Uses only `tokio` (already in deps) — no axum, no wiremock.

### Tier 4 — Render helper unit tests (no TestBackend snapshots)

- [x] Extract `event_summary(record: &EventRecord) -> String` as a `pub(crate)`
  function in `ui.rs`; unit-test all four event types produce expected strings

- [x] Extract `is_diagnostic(event_type: &str) -> bool` (filters `tool_call_*`
  events for Diagnostics view); unit-test the filter

- [ ] (Optional) One "does not panic" `TestBackend` smoke test: render each
  view with an empty `App` and a populated `App` into an 80×24 buffer, assert
  no panic. Do NOT build golden-snapshot comparisons.

---

## File Map

```
crates/curator/
├── Cargo.toml             slice 1  (add deps)
└── src/
    ├── msg.rs             slice 1  (new: Msg, SseStatus)
    ├── input.rs           slice 2  (new: input thread)
    ├── event.rs           slice 2  (extend Action enum)
    ├── sse.rs             slice 3  (new: SSE thread + LineBuffer + next_backoff)
    ├── app.rs             slice 4  (extend App, push_event, SessionSummary)
    │                                + TDD push_event tests
    ├── main.rs            slice 5  (CLI flag, thread wiring, new run loop)
    ├── ui.rs              slice 6  (view dispatch, activity, diagnostics)
    │                                + event_summary / is_diagnostic helpers
    └── tui.rs             (unchanged)

Tests (slice 8):
    app.rs    — push_event, priority sort, reload_kanban, next_backoff
    sse.rs    — LineBuffer unit tests (chunk boundary, keep-alive, cursor, …)
    ui.rs     — event_summary, is_diagnostic helpers; optional TestBackend smoke
```

---

## Deferred (out of scope for this plan)

Per the Phase 6 MVP spec (`spec/curator/ui.md`):
- `e` — open selected artifact in `$EDITOR` with auto-rescan
- `f` — filter panel
- Multi-project toggle (single ↔ all registered projects)
- Collapsing/drilling into sessions → turns → tool calls
- Fuzzy search
