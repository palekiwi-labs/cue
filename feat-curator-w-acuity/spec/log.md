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

