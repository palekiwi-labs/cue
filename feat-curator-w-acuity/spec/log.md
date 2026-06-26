# Project Log

## [641bf3b] Phase 6 Slice 1 complete — Msg enum and deps wired

Implemented Phase 6 Slice 1 of curator live half plan.

- **Found:** tokio and reqwest were already in the workspace Cargo.lock via acuity crate — zero new transitive deps added to build graph
- **Found:** acuity-api crate exposes EventRecord and re-exports AcuityEvent from acuity-schema — exactly what msg.rs needs
- **Found:** Two expected dead_code warnings for Msg and SseStatus will resolve when wired in Slices 2-5
- **Decided:** Added futures-core explicitly to Cargo.toml since it is used in SSE streaming (stream combinator); plan only listed it as a note
- **Decided:** Declared msg module in main.rs to make the crate compile cleanly with new types in scope

## [4abe376] Phase 6 Slice 2 complete — input thread and Action extensions

Implemented Phase 6 Slice 2: crossterm input thread and Action enum extensions.

- **Found:** View enum had to be added to app.rs as a prerequisite for Action::SwitchView(View) — no conflict with Slice 4 additions which only add fields/methods to the App struct
- **Found:** event.rs importing View from app.rs creates no circular dependency since app.rs does not import from event.rs
- **Found:** The old next_action() run loop in main.rs needed a catch-all arm for SwitchView/Refresh — kept as no-ops pending Slice 5 unified loop
- **Decided:** Kept next_action() and the old run loop fully intact as directed by the plan — input thread and new Action variants are additive only
- **Decided:** map_key() in input.rs is a private helper (not pub) since it is only called within the spawn closure — keeps the module surface minimal

## [4abe376] Plan amended per Opus review — Slice 8 added, LineBuffer extraction required

- **Found:** SSE parser is pure/synchronous logic once extracted — zero infrastructure needed to test chunk-boundary bugs, keep-alive skip, cursor tracking
- **Found:** next_backoff and LineBuffer should be extracted as pure helpers so Slice 8 can unit-test them
- **Found:** TestBackend snapshot tests have negative ROI while UI is in flux — only a does-not-panic smoke test is warranted
- **Found:** Tier 3 end-to-end test only needs tokio::net::TcpListener already in deps — no axum or wiremock needed
- **Decided:** Added Slice 8 as dedicated automated test slice — keeps Slice 7 as cleanup+manual-QA only
- **Decided:** Slice 3 must extract LineBuffer struct and next_backoff fn for testability
- **Decided:** push_event tests written red-green in Slice 4 (TDD), expanded in Slice 8
- **Decided:** Manual QA checklist stays in Slice 7 — targeted at cursor-resume correctness (step 7) which automated tests cannot fully cover end-to-end

## [30fb8fc] Phase 6 Slice 3 complete — SSE thread with LineBuffer and next_backoff

- **Found:** futures-util (not futures-core) provides StreamExt needed for stream.next().await — swapped dep in Cargo.toml, already in lockfile so no new transitive deps
- **Found:** Connected status sent inside connect_and_stream right after 200 response (not after stream end as plan originally stated) — better UX: UI shows connected during active streaming
- **Decided:** LineBuffer extracted as pure synchronous struct per Opus recommendation — all parse logic (chunk-boundary, keep-alive skip, cursor tracking) testable without server in Slice 8
- **Decided:** next_backoff extracted as pure pub(crate) fn for trivial unit testing of 5000ms cap
- **Decided:** Deviated from plan's Connected-after-Ok placement: Connected sent immediately after HTTP 200 inside connect_and_stream; run_loop only resets cursor/backoff on Ok return

## [dfc440b] Phase 6 Slice 4 complete — App extended with live state and push_event tests

- **Found:** acuity_schema individual types (SessionIdle, AgentTurnCompleted, etc.) are not re-exported from acuity_api — added acuity-schema as dev-dependency to curator for test helpers
- **Found:** All 7 push_event tests passed GREEN immediately: eviction cap, oldest-first eviction, token accumulation, project-dir-survives-eviction, error counting, session_idle metadata, diagnostics_len filter
- **Found:** classify_tasks extracted as private fn to enable reload_kanban reuse without duplicating the classification logic
- **Decided:** AcuityStatus mirrors SseStatus with a From<SseStatus> impl so process_msg in Slice 5 can write app.acuity_status = s.into() cleanly
- **Decided:** EVENT_CAP = 2000 exposed as pub const so Slice 8 tests can use it without hardcoding the magic number
- **Decided:** diagnostics_len() computes on the fly (no cached counter) — simple and correct, re-evaluated each scroll

## [1b89314] Phase 6 Slice 5 complete — unified run loop and main.rs wiring

Replaced the old poll-based run loop with a unified rx.recv() + try_recv() drain loop. Wired input and SSE threads. Added --acuity-url / $ACUITY_URL CLI arg. All 7 curator tests and full workspace test suite green.

- **Found:** clap env feature was not enabled — added 'env' to features list in Cargo.toml to support #[arg(env = "ACUITY_URL")]
- **Found:** process_msg must check LoopControl::Quit on both the initial rx.recv() AND the try_recv() drain — the plan's pseudocode only checked the drain loop, which would miss Quit from the first message in a batch
- **Found:** Two pre-existing dead_code warnings remain: next_action() (removed in Slice 7) and SessionSummary session_id/first_seen fields (used in Slice 6 rendering)
- **Decided:** Added root and branch as parameters to run() and process_msg() so reload_tasks() can call read_artifacts() without global state
- **Decided:** LoopControl checked symmetrically in both recv() and try_recv() branches to ensure Quit is never missed
- **Decided:** Disabled (kanban-only) mode sends SseStatus::Disabled synchronously before run() starts so App.acuity_status is correct from the first draw

## [73e62e2] Phase 6 Slice 6 complete — view dispatch, Activity Feed, Diagnostics

Added three-view UI to curator. render() now dispatches on app.active_view. render_kanban() renamed from render(). render_activity() and render_diagnostics() added. Priority sort added to classify_tasks(). event_summary() and is_diagnostic() exported pub(crate) for Slice 8 tests. All workspace tests green.

- **Found:** sel_activity is an index into events[], not the visual list — headers injected between session groups shift the visual position; computed by tracking items.len() before appending each event row
- **Found:** status_help_line() returns Line<'static> using owned Strings from format!() — Span::styled with an owned String resolves to 'static lifetime via Cow::Owned
- **Found:** layout_with_help() extracted as shared helper used by all three views
- **Decided:** Priority sort applied in classify_tasks() so both App::new() and reload_kanban() benefit without duplication
- **Decided:** Diagnostics view deserializes payload per-row (only tool_call_* events) — acceptable cost since the filter is cheap and the list is bounded by EVENT_CAP
- **Decided:** acuity_status_parts() returns (String, Color) tuple to avoid lifetime complexity on the status Span helper

## [52182bd] Phase 6 Slice 7 complete — cleanup, clippy clean

Removed next_action() from event.rs (dead since Slice 5). Fixed three clippy lints: collapsible_if in app.rs, two while_let_loop patterns in input.rs and sse.rs. Added #[allow(dead_code)] to SessionSummary.session_id and .first_seen (populated for Slice 8 tests). cargo clippy --workspace -- -D warnings clean. All tests green.

- **Found:** Clippy found three additional lints not caught during Slice 4/5 development: collapsible_if in push_event, while_let_loop in input.rs spawn closure, while_let_loop in LineBuffer.feed()
- **Found:** SessionSummary.session_id and .first_seen are pub fields on a non-pub-library type, so rustc correctly warns about them as dead code even though they are populated
- **Decided:** #[allow(dead_code)] with explanatory comment on those two fields rather than removing them — they are part of the Slice 8 test API
- **Decided:** Slice 7 automated checks complete; manual QA checklist (requires live acuity) deferred to human

## [1c74d35] Phase 6 Slice 8 complete — automated test coverage

Added 21 new tests across three modules. All 28 curator tests pass. Tier 1 (App state): priority_sort_in_app_new, reload_kanban_reclassifies_resets_selection_and_sorts. Tier 2 (LineBuffer): 11 tests covering single event, chunk boundary, keep-alive skip, leading-space strip, malformed JSON, multiple events, blank-line no-op, cursor tracking. Also next_backoff doubles-then-caps. Tier 4 (render helpers): 6 event_summary tests (all 4 event types incl. error/ok completed), 1 is_diagnostic test.

- **Found:** All 11 LineBuffer tests passed GREEN on first run — confirms the Slice 3 implementation was correct with no edge-case bugs discovered
- **Found:** record_json helper using Rust raw strings with {{}} escape for braces works cleanly without needing serde to serialize EventRecord
- **Found:** cursor_not_updated_after_malformed_json test confirms cursor only advances on successful parse — important for reconnect correctness
- **Decided:** Tier 3 (thin end-to-end wiring test with TcpListener) skipped per plan's 'optional' designation — Tier 1+2 cover ~90% of risk
- **Decided:** TestBackend smoke tests skipped per plan — negative ROI while UI is in flux

## [1c74d35] Review fixes plan updated; Slice 2 todo created

- **Found:** The existing REST API is forward-only (after+limit ascending) — no max_seq endpoint and no descending order. Pure client-side 'last N events' fetch requires paginating from 0, which defeats the purpose. A server affordance is needed.
- **Found:** On first connect with Last-Event-ID: 0, the acuity server replays the entire database with no total cap — only paginated at 50/page x 10 pages per 500ms cycle. EVENT_CAP=2000 bounds client storage but does not prevent the server from sending tens of thousands of events.
- **Found:** Session summaries (project_dir, token totals, error counts) are a cumulative fold — any history window makes them approximate for sessions older than the window. This is unavoidable with any replay limit and is the accepted trade-off for a live-monitoring TUI.
- **Decided:** Ship review fixes (M3, M1, H2, C1, H1) first as the merge unblocker. C1 try_send is load-bearing until Slice 2 ships.
- **Decided:** Adopt Option D (server-side ?limit_history=N) for Slice 2 — the only approach feasible within the existing forward-only query API that avoids extra round-trips.
- **Decided:** N = 3000 (EVENT_CAP=2000 + 1000 headroom) so active sessions' session_idle is almost always within the window.
- **Decided:** C1 try_send remains in place after Slice 2 ships — it guards against reconnect bursts and live overload, not only cold-start history.
- **Decided:** Updated review-fixes.md: reframed C1 rationale from cold-start backlog to reconnect/overload; added Slice 2 relationship note to Foreword.
- **Decided:** Created todo sse-limit-history.md with full implementation spec for a future agent.

## [3f984e4] [3f984e4] Review fixes complete — all 5 pre-merge findings addressed

Applied all 5 findings from the Phase 6 code review (review-fixes.md). Two files changed: sse.rs and main.rs. 29 curator tests pass, workspace clippy clean.

- **Found:** All three eprintln! sites in sse.rs were straightforward removals — no silent failures, SseStatus::Reconnecting already covers UI surfacing
- **Found:** data_multiline_lines_joined_with_newline test passed immediately on first run — the split at a JSON key boundary (between two top-level fields) produces valid JSON when joined with LF
- **Found:** drop(tx) in main() makes the Err(_) arm in run() reachable for the first time — previously tx was never dropped so recv() could never return Err during a live run
- **Decided:** M3: removed all eprintln! calls from SSE thread (spawn + run_loop + LineBuffer::feed) — TUI safety trumps debug output
- **Decided:** M1: reqwest::Client moved to run_loop top-level — shared across reconnects
- **Decided:** H2: LF join guard (if !self.pending_data.is_empty()) applied before push_str; new test added (29 total)
- **Decided:** C1: tx.send() replaced with tx.try_send() for Msg::Sse only; SseStatus messages remain blocking send() — they are low-frequency and UI-critical
- **Decided:** H1: drop(tx) added in main() after all thread spawns; Err(_) arm changed to return Ok(()) with explanatory comment; trailing Ok(()) removed from run() (infinite loop, all paths return)

## [05eeca7] [05eeca7] Opus review follow-up — clarified misleading M3 comment

Addressed Opus review follow-up #1 (the only "should fix" item). The other three items (C1 cursor coupling note, H2 test contract comment, M1 Client::builder symmetry) were classified as nits/optional and left for a future touch-up.

- **Found:** Opus confirmed the underlying code is correct — only the comment's framing oversold the safety; the silent-skip behaviour itself is appropriate for a server-controlled wire format
- **Decided:** Rewrote sse.rs:109-110 comment to state plainly that malformed-JSON drops are unobservable to the UI (the caller's for-loop just skips an empty vec) and that this is accepted because acuity owns the wire format; dropped the false claim that 'zero records returned signals the drop'
- **Decided:** Left the C1/H2/M1 nit items unaddressed — all classified as documentation/test-hygiene improvements rather than merge blockers

