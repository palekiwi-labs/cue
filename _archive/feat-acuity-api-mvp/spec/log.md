# Project Log

## [03f7817-dirty] Slice 1 — acuity-api types defined

Commit 03f7817: acuity-api/src/lib.rs populated with EventRecord, EventsPage, and a re-export of AcuityEvent from acuity-schema.

- **Decided:** Re-export AcuityEvent from acuity-api so curator needs only one crate dependency for both response types and payload parsing
- **Decided:** payload field stays as raw String — curator calls serde_json::from_str::<AcuityEvent> when it needs structured data
- **Decided:** acuity-api stays dependency-light: only serde, serde_json, acuity-schema

## [988f9da-dirty] Slice 2 — query_events_after added to db layer

Commit 988f9da: db::query_events_after uses sqlx::QueryBuilder for optional session_id/event_type filters, clamps limit to 1..=500, orders by seq ascending.

- **Decided:** Use QueryBuilder for optional AND-combined equality filters (parameterized, no injection risk)
- **Decided:** Clamp limit 1..=500 inside db function, not at the handler boundary
- **Decided:** Row mapping done manually (sqlx::Row::get) since acuity-api must stay sqlx-free

## [ec33f38-dirty] Slice 3 — GET /events query endpoint green

Commit ec33f38: 8 new tests for the query endpoint all pass. EventsQuery extractor handles after/limit/session_id/event_type params. SSE handler also wired in this commit (sse_handler skeleton needed for router compilation).

- **Found:** axum get().post() method chaining does not require importing post separately — routing::get is sufficient
- **Found:** async-stream + futures-core are the minimal deps for SSE stream implementation in axum 0.8
- **Decided:** after param normalised to max(0) at handler boundary, limit clamped inside db::query_events_after
- **Decided:** On DB error in query_events handler: log and return empty page (200) rather than 500, consistent with fire-and-forget philosophy

## [a8d0fb9-dirty] Slice 4 — SSE endpoint green, all workspace tests pass

Commit a8d0fb9: GET /events/stream implemented with drain-first poll loop, explicit 15s keep-alive, defensive Last-Event-ID parsing, logged DB errors. Two SSE smoke tests added (first-frame assert, resume-from-cursor). All 30 acuity tests + full workspace green. Clippy clean.

- **Found:** Broken doctest in acuity-api/src/lib.rs (used ? without return type) — fixed by converting the example to prose
- **Found:** async_stream::stream! + futures_core::Stream is the correct pattern for axum 0.8 SSE without a separate tokio-stream dep
- **Found:** axum SSE body can be read frame-by-frame in tests via http_body_util::BodyExt::frame()
- **Decided:** poll-based SSE confirmed: drain-first inner loop prevents burst lag; sleep only after short page
- **Decided:** Keep-alive explicit: KeepAlive::new().interval(Duration::from_secs(15)) matches plan spec

## [a8d0fb9-dirty] Phase 5 complete — acuity-api read model validated, task closed

All three acceptance criteria for task acuity-read-model.md are verified and the task is marked complete.

AC #1 (query endpoint): validated by bin/1782211100-a8d0fb9/validate-phase5.sh — all 8 checks green (server health, seed 4 event types, unfiltered query, session_id filter, event_type filter, after-cursor pagination, limit cap, EventRecord field completeness).

AC #2 (SSE stream): validated by bin/1782211100-a8d0fb9/validate-phase5-sse.sh — all 7 checks green (server health, live delivery of all 4 event types, Last-Event-ID resume with cursor respected).

AC #3 (independent of curator): human-attested. Both endpoints exercised via raw curl with zero curator involvement.

Task acuity-read-model.md status -> complete, branch cleared. Executive plan status -> complete. Phase 5 of the acuity roadmap is ready to merge.

- **Found:** SSE validation script confirms drain-first poll loop delivers all 4 event types within the 3s window and that Last-Event-ID resume correctly skips events at/before the cursor seq
- **Decided:** Close task acuity-read-model.md as complete; all three acceptance criteria verified with evidence
- **Decided:** Phase 5 ready to merge to master on completion of the feat/acuity-api-mvp PR

## [a8d0fb9-dirty] Curator UI/UX specification drafted from Opus MVP consultation

Replaced the stub at .cue/master/spec/curator/ui.md with a full UI/UX specification synthesising the brainstorming discussion and the Opus MVP-scoping consultation.

The spec covers three gitui-style views (1=kanban, 2=activity, 3=diagnostics) with each view split into MVP and Deferred sections, plus global keybindings, configuration, and acuity-api evolution notes.

Key decisions encoded:
- Curator is read-only; the only mutation side-channel is $EDITOR launch with auto-rescan on close.
- Kanban MVP: task artifacts only (master), three status columns (Open/In-Progress/Complete, Closed excluded), status + priority filters, fixed priority-desc sort, multi-project toggle.
- Activity feed MVP: single SSE connection (GET /events/stream with no Last-Event-ID) drains the full backlog then tails live; client-side session derivation via HashMap fold; ring buffer cap (~2000); reconnect-with-cursor.
- Diagnostics MVP: live tool-call list reusing the same SSE stream, success/error distinction from deserialized payload (no schema change).
- acuity-api-mvp PR should merge as-is; all future additions (sessions endpoint, since param, aggregations) are additive. tool_name promotion to top-level field is explicitly rejected.
- Trigger conditions recorded for promoting deferred items (fuzzy search, collapsing/drilling, /sessions, since param).

The decisive insight: the existing SSE drain-then-tail behaviour (acuity/src/main.rs:130-160) collapses the activity-feed data-arrival design into a single connection that replays history then tails live, removing any merge-blocking dependency on acuity-api.

- **Found:** acuity's SSE handler (crates/acuity/src/main.rs:130-160) already drains the full backlog from seq > 0 then tails live when Last-Event-ID is absent/0, so a single connection delivers both history and live updates
- **Found:** Session state is fully derivable client-side from the event stream: identity/project from SessionIdle, span from received_at, tokens from AgentTurnCompleted, errors from tool_call_completed.is_error
- **Found:** acuity has no index on received_at (crates/acuity/src/db.rs:12-25 indexes only session_id and turn_id), so a since=timestamp filter would be a full scan today
- **Found:** cuelib::project::ProjectStore already provides the multi-project registry; the kanban multi-project toggle is wiring, not new infra
- **Found:** The existing curator/src/app.rs scaffold already classifies tasks into Open/In-Progress/Complete and excludes Closed
- **Decided:** Curator is read-only; $EDITOR is the only mutation side-channel with auto-rescan on close
- **Decided:** Kanban MVP limited to task artifacts on master; Closed tasks excluded from the board
- **Decided:** Activity feed uses a single SSE connection that drains backlog then tails live; session state derived client-side (HashMap fold), no /sessions endpoint needed for MVP
- **Decided:** Diagnostics reuses the same SSE stream as the activity feed; tool_name/is_error read from deserialized payload, no schema change
- **Decided:** Merge acuity-api-mvp PR as-is; all future acuity additions are additive
- **Decided:** Reject promoting tool_name to a top-level EventRecord field (breaking + contradicts row-mirrors-column design)
- **Decided:** Fixed priority-desc ordering in MVP; fuzzy search and user-selectable sort deferred to Phase 6
- **Open:** Ring buffer cap value (2000) is a placeholder to be tuned against real event volumes
- **Open:** Whether Enter should stay reserved or be left unbound in kanban MVP until card detail lands

## [4f292f4-dirty] Slice 1 done: explicit pagination cursor + workspace rustfmt

Committed Slice 1 of the acuity review fixes on feat/acuity-api-mvp.

Commit 79a5aa8 (style): applied rustfmt workspace-wide. This resolved
long-standing fmt drift across cue/cuelib/acuity that had been
persisting across sessions and blocking clean commits. The user
confirmed this was the right call -- the debt would keep accumulating
otherwise. 8 files, formatting-only.

Commit 4f292f4 (fix): Critical #1 -- EventsPage now exposes
next_after: Option<i64>. query_events_after returns
(Vec<EventRecord>, Option<i64>). The server owns the "is there more?"
decision so clients never depend on the server's internal page-size
clamp. Killed the latent silent-data-loss bug for any future consumer
requesting limit > 500. 3 new tests (None on short page, Some on full
page, full cursor round-trip). 33 acuity tests pass.

- **Found:** Both gemini-flash subagents (consultant-gemini-flash, diff-reviewer-gemini-3.5-flash) return empty result bodies in this environment -- a systemic issue, not a prompt problem. Avoid relying on them; use opus/sonnet instead.
- **Found:** Branch HEAD had 16 pre-existing rustfmt violations -- fmt was never run during the original Phase 5 work.
- **Decided:** Approach for fmt debt: committed a dedicated style commit (user endorsed) rather than hand-formatting only edited regions. Tree is now fmt-clean going forward.
- **Decided:** Pagination fix: explicit next_after cursor (chosen earlier, now implemented) over returning the effective limit.

## [7de1560] Acuity opus-review fixes complete (all 4 slices)

Completed all four slices of the acuity opus-review fix task. The branch
feat/acuity-api-mvp now addresses both critical bugs and the agreed
partial SSE hardening + cargo hygiene. Task marked complete; all six
acceptance criteria have evidence.

Commits (in order):
- 79a5aa8 style: apply rustfmt across workspace (pre-existing debt cleanup)
- 4f292f4 fix: explicit pagination cursor next_after (Critical #1)
- c753114 fix: surface query DB errors as 500 (Critical #2)
- 11f7d33 refactor: bound SSE drain loop + content-type test + helper (Major #3/#4/#5)
- 7de1560 chore: drop unused serde_json dep + sync stale Cargo.lock

Final state: fmt clean, clippy -D warnings clean, all workspace tests
pass (35 acuity + all others). 4 new tests added (3 pagination cursor +
1 db-failure 500 + 1 SSE content-type = 5 new total).

- **Found:** Cargo.lock on this branch was stale relative to its own Cargo.toml files: Phase 5 added acuity-api/async-stream/futures-core to acuity and acuity-schema to acuity-api, but the lockfile was never regenerated. Caught and synced in 7de1560.
- **Found:** Both gemini-flash subagents (consultant + diff-reviewer) return empty result bodies in this environment -- a systemic issue. Avoid relying on them; opus/sonnet work fine.
- **Decided:** For the recurring rustfmt debt: user endorsed committing a workspace-wide style commit (79a5aa8) to stop the drift accumulating across sessions, rather than hand-formatting only edited regions. This unblocked clean commits going forward.
- **Decided:** DB-error response shape: plain StatusCode 500, no JSON body -- matches the existing bare-StatusCode style of handle_event (400/422/500).
- **Open:** Deferred follow-ups captured in todo/1782231988-7de1560/acuity-sse-deferred-followups.md: broadcast-channel SSE redesign, disconnect/no-leak test, cosmetic nits. Low priority.

## [7de1560] Opus verification review: ADDRESSED, branch merge-ready

Dispatched a verification review to consultant-opus (the same model that
did the original review). It read the original review trace
(trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md) and the saved
fix diff (tmp/review-fixes-a8d0fb9-to-HEAD.diff), and verified each
finding.

VERDICT: ADDRESSED -- all blocking findings correctly resolved,
deferrals acceptable for merge, no regressions.

Saved the verification review as
trace/1782231988-7de1560/opus-verification-review-acuity-api-mvp.md.

One non-blocking residual noted: the exact-boundary case (full page
where remaining rows == clamped_limit, then resume yields a literally
empty page + None) is covered by logic + comment but lacks a dedicated
test asserting events.len() == 0. Opus flagged the disconnect test as
the "weakest deferral" but still acceptable.

- **Found:** opus verification is a reliable reviewer in this environment (unlike the gemini-flash agents which return empty).
- **Decided:** Branch is merge-ready per opus verification. No further fixes required for the critical/major findings.
- **Open:** Optional follow-up: add the exact-boundary empty-resume test (minor, non-blocking). The disconnect test remains the priority follow-up per the deferred todo.

