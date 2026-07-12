# Project Log

## [ae09f03] Slice 5 complete — ActivityItem and push_event fix shipped

Two commits on feat/curator-activity-item:

1. fix: set project_dir from all events in push_event (5553240)
   - push_event now updates SessionSummary.project_dir unconditionally from
     EventRecord.project_dir on every event (not just session_idle).
   - Removed redundant project_dir set from the session_idle match arm.
   - Added regression test: non-idle first event still sets project_dir.

2. feat(curator): ActivityItem enum and build_activity_items (ae09f03)
   - New crates/curator/src/activity.rs module.
   - ActivityItem<'a> enum with SessionHeader, Turn, Standalone variants.
   - build_activity_items pure function with HashMap<(&str, &str), usize>
     turn_map (borrow-keyed, no per-event clone).
   - 7 unit tests covering all Opus-flagged failure modes.
   - mod activity wired into main.rs (not yet called from ui.rs).

- **Found:** Test seq ordering matters: agent_turn_completed always arrives with higher seq than its tool calls in the real event stream. Initial tests had the ordering reversed, causing turn_map lookups to miss and tool calls to fall through to Standalone. Fixed by reordering events in the affected tests to put agent_turn at the highest seq.
- **Found:** The existing project_dir_survives_ring_buffer_eviction test expected /my/project after evicting the idle, but the new unconditional update means subsequent turns (carrying /home/pl/code) overwrite it. Updated the expectation to match the new correct behavior.
- **Found:** Box::leak in test helpers is a viable strategy for giving ActivityItem<'a> a 'static lifetime in unit tests without restructuring the entire test module around owned data.
- **Decided:** Use HashMap<(&'a str, &'a str), usize> for turn_map (Opus recommendation) — avoids per-event String clone on insert and lookup.
- **Decided:** Suppress folded tool calls rather than emitting as Standalone; Standalone is reserved for true orphans (parent evicted from ring buffer).
- **Decided:** Allow dead_code on ActivityItem enum and build_activity_items fn — these will be wired into ui.rs in Slice 6.

## [ae09f03] Review findings consolidated into pre-Slice-6 fix plan

Dual code review (GLM + Opus) of feat/curator-activity-item diff, followed by Opus consultation on the resulting plan. Findings consolidated into an executive plan at .cue/feat-curator-activity-item/plan/1782657928-ae09f03/slice6-review-fixes.md.

- **Found:** fold_state.contains() allocates 2 Strings per tool call per frame — hot-path in reverse-chrono iteration
- **Found:** Duplicate turn_id silently attaches tool calls to older turn (turn_map.insert overwrites, second-seen in reverse-chrono wins, but second-seen is the older event)
- **Found:** build() test helper returns VecDeque::new() with a false ownership comment — the leaked deque is the real lifetime anchor
- **Found:** turn_id: None branch tests are missing for both agent_turn_completed and tool_call_* arms — these are column-driven defensive tests, constructible via make_record(... None ...)
- **Found:** or_insert_with (not or_insert) is the correct form: moves String allocation inside the closure so duplicate turn_id path pays zero heap alloc
- **Found:** Duplicate agent_turn_completed still renders as a second Turn item — or_insert_with only deduplicates which Turn receives tool calls, not which Turn is pushed to items
- **Found:** SessionSummary.project_dir is now latest-non-empty scalar and can disagree with per-event header dir — log-only, no code change
- **Decided:** Use or_insert_with with expanded computation inside the closure to avoid wasted allocations on duplicate turn_id path
- **Decided:** Accept double-rendering of duplicate turn_id as Turn items — retries with same turn_id are not expected in normal acuity usage, and the losing duplicate cannot receive tool calls
- **Decided:** SessionSummary.project_dir = latest non-empty is the explicit contract — no code change, log entry suffices
- **Decided:** Box::leak in test helpers is acceptable (bounded per-run leak) — update the false doc-comment
- **Decided:** Commit 1 uses fix() type (not refactor) since the or_insert_with change is a bug fix for tool-call mis-routing

## [0c2ff37] Review fixes shipped — activity.rs 42/42 tests green

Implemented all pre-Slice-6 fixes from the dual code review. Two commits on feat/curator-activity-item:\n\n1. fix(curator): hoist fold check to turn-insertion, fix duplicate turn_id (3976d5c)\n2. test(curator): add edge-case tests, simplify build() helper (0c2ff37)

- **Found:** or_insert_with (not or_insert) was the correct form: the String allocation for the fold check lives inside the closure, skipped entirely on duplicate turn_id path
- **Found:** git add -p cleanly split behavioral changes (hunks 1-3) from test changes (hunks 4-9) in a single-file commit
- **Decided:** SessionSummary.project_dir contract is latest-non-empty scalar — logged here, no code change needed
- **Decided:** Duplicate agent_turn_completed records render as two Turn items but only the newest wins the map slot; accepted as the correct behavior for the prototyping stage

## [0c2ff37] Lineage collection design locked — Slice 6a (Option L) planned

Investigated why the curator activity feed rendered incoherently (repeated identical `cue (ses_0f14)` headers, fragmented groups). Used a real SQLite snapshot (465 rows) as ground truth and read the opencode source. Designed the collection fix via an Opus consultation; user approved. Created two executive plans (6a collection, 6b rendering) and one todo (sessions-table normalization).\n\nPipeline: opencode-plugin → acuity server (POST /events) → SQLite → curator (SSE). Plugin at `~/.config/opencode/plugin/palekiwi-labs/cue-plugins/src/opencode/acuity-plugin.ts`.\n\nRoot cause = COLLECTION-side. opencode's Session.Info has parentID/agent/model/title (`ref/opencode/packages/core/src/session/schema.ts:27-46`); the Task tool sets `parentID: ctx.sessionID` (`task.ts:129-145`). The plugin captured NONE of it — only handled session.idle (title only), message.updated, message.part.updated. Sub-agent sessions are distinct sessions that run concurrently and interleave by arrival time; 3 distinct `ses_0f14*` sessions collapsed to the same 8-char label via `ui.rs:214` truncation.\n\nTitle arrived only at idle because session.idle was the sole reader. opencode regenerates the title seconds after the first prompt via ensureTitle (`prompt.ts:226-285`) → setTitle → patch → publishes session.updated with full info (`session.ts:776-788`). Adding a session.created/updated handler gets title early + parentID + agent + model in one stroke.

- **Found:** Sub-agent sessions are distinct sessions with their own session_id and a structural parent_id (task.ts:129-145); they run concurrently and interleave by arrival time. Confirmed via DB: 3 distinct ses_0f14* sessions in one 4-min window, opus+glm active in the same minutes.
- **Found:** ALL 465 events share the same project_dir (/home/pl/code/palekiwi-labs/cue) because the plugin hardcodes project_dir:directory from load-time context (acuity-plugin.ts:79,100,129,148). So the (session_id, project_dir) header key in Slice 5's build_activity_items degenerates to session_id only — a known issue deferred to stage C.
- **Found:** The 8-char session-id truncation (ui.rs:214) collides: opencode IDs share a per-run prefix ses_0f14, so 5 distinct sessions render identically. Pure display bug that hid the real situation.
- **Found:** session.created/session.updated events carry properties.info: Session with parentID/title (types.gen.ts:533-547,562-574); agent/model are in the runtime object though the V1 SDK type may omit them (access defensively).
- **Found:** The real parent->child tree edge for stage C is the parent's Task-tool-completed result text <task id="<childSessionID>"> already persisted in the payload column. parent_id is the clean child->parent back-edge. Both edges available without new columns.
- **Found:** project_dir/harness denormalization onto every events row is debt: duplicates invariants, grows the column-mismatch guard (db.rs:57-78), and sets a precedent that nearly drove Option V.
- **Decided:** Option L (lean): add one new acuity event variant SessionUpdated {session_id, project_dir, harness, parent_id, agent, model, title} emitted by the plugin on opencode session.created + session.updated. Curator ingests into SessionSummary. NO new DB columns, NO changes to existing 4 variants. Chosen over Option V (denormalize parent_id/agent/model per row) because the display path already joins into sessions HashMap, SessionSummary already survives ring-buffer eviction by design, and per-row parent_id is both redundant (invariant per session) and insufficient for stage-C turn-level tree placement.
- **Decided:** model stored as flat string "providerID/modelID".
- **Decided:** SessionUpdated carries the project_dir/harness envelope (unlike Opus's sketch) to keep enum accessors exhaustive and insert_event NOT NULL binds valid.
- **Decided:** parent_id is set-once-if-some in SessionSummary (guard against later null clobbering a known parent, mirroring app.rs:203); title/agent/model/harness are last-writer-wins.
- **Decided:** Do NOT normalize to a sessions table now (deferred to todo normalize-events-sessions-table.md). Trigger to action: adding the planned 'pi' harness, persistent session summaries, or cross-harness aggregation. The sessions-table migration makes 'pi' a value addition (no schema change) rather than a structural event.
- **Decided:** Slice split: 6a = collection (schema variant + plugin handlers + curator push_event/SessionSummary ingest); 6b = stage-B rendering (kill 8-char truncation, title+agent+lineage headers, debug columns).
- **Open:** If 6a live testing reveals lineage gaps for sessions that never fire session.updated, fallback is to also emit SessionUpdated from the idle handler.
- **Open:** activity.rs build_activity_items header key (session_id, project_dir) is degenerate and needs parent_id-based rework in stage C (nested tree).
- **Open:** 6b plan is intentionally light; will be fleshed out when 6a live data is in hand.

## [7f4bac5] Slice 6a step 1 — SessionUpdated schema variant shipped

Added the SessionUpdated event variant to acuity-schema (commit 7f4bac5). This is step 1 of slice 6a (collection layer for lineage display).

New SessionUpdated struct: session_id, project_dir, harness (envelope), parent_id/agent/model/title (all Option<String>). Added the enum arm plus all 5 accessor match arms (event_type -> "session_updated", session_id, project_dir, harness, turn_id -> None). Compiler exhaustiveness mandated all arms at once.

5 new unit tests (round-trip, discriminant, turn_id None, raw-JSON deserialize, all-optional-null round-trip); extended the 3 "all variants" accessor tests. Schema suite now 25/25 green. clippy clean.

Verified ts_rs codegen emits the SessionUpdated interface and adds {type:"session_updated"}&SessionUpdated to the AcuityEvent union — confirming the plugin (step 2-3) will get correct types.

- **Decided:** parent_id/agent/model/title are all Option<String> on the wire — primary sessions carry null lineage
- **Decided:** SessionUpdated.turn_id() returns None (no turn concept for a session-level event)
- **Decided:** Field order: session_id, project_dir, harness, parent_id, agent, model, title — envelope first, lineage last

## [1acf52f] Slice 6a step 4-5 — curator SessionSummary ingest shipped

Implemented step 4-5 of slice 6a: curator ingest of SessionUpdated into SessionSummary (commit 1acf52f).

SessionSummary gained 4 fields: harness (String), parent_id/agent/model (Option<String>). Initialized in or_insert_with (harness from record.harness.clone(), others None). New "session_updated" match arm in push_event deserializes the payload and applies: harness last-writer-wins, title/agent/model last-writer-wins-only-when-Some, parent_id set-once-if-some (guard: entry.parent_id.is_none() && ev.parent_id.is_some()).

5 new curator tests: push_session_updated_populates_summary, parent_id_set_once_not_clobbered_by_later_null, title_updates_last_writer_wins, agent_and_model_populated, session_updated_optional_none_leaves_fields_none. Curator suite now 47/47 green. Full workspace test + clippy clean.

Verified the acuity server accepts session_updated events generically (insert_event uses the enum accessors, now extended — no server changes needed). The server's 39 tests pass unchanged.

- **Decided:** harness initialized from record.harness.clone() in or_insert_with (every session gets it immediately, not just those with a session_updated event)
- **Decided:** parent_id guard: entry.parent_id.is_none() && ev.parent_id.is_some() — first known parent wins, later nulls never clobber
- **Decided:** title/agent/model use if-let-Some guards so a None in the payload never overwrites a previously-known value
- **Decided:** New SessionSummary fields are pub without #[allow(dead_code)] — clippy is clean because the crate structure (pub fields on pub struct) suppresses the lint, matching existing fields like session_title

## [0b0aa5b] Slice 6a steps 2-3 — plugin SessionUpdated handlers shipped

Completed steps 2-3 of slice 6a: regenerated TypeScript types and added the session.created/session.updated handlers in the cue-plugins repo (commit 8555572).

Ran the ts_rs codegen binary (cargo run -p acuity-schema --bin codegen -- <cue-plugins>/src/generated/acuity) to regenerate types.ts. Confirmed SessionUpdated interface and the {type:"session_updated"}&SessionUpdated union member are emitted.

Added a combined session.created || session.updated handler to acuity-plugin.ts, placed before the existing session.idle handler. Extracts from event.properties.info (Session): session_id=info.id, parent_id=info.parentID ?? null, title=info.title || null. agent/model are absent from the V1 SDK Session type but present at runtime — accessed via typed casts (info as { agent?: string }).agent and (info as { model?: unknown }).model cast to {providerID?, modelID?}, flattened to "providerID/modelID" only when both present. project_dir=directory, harness="opencode" (same envelope as all other events). session.idle handler left unchanged.

Typecheck (tsc --noEmit via nix develop -c) passes cleanly.

The existing session.idle handler is unchanged — it still fetches the session via client.session.get and emits SessionIdle. Per the plan's fallback note, if live testing reveals lineage gaps for sessions that never fire session.updated, we can also emit SessionUpdated from the idle handler.

- **Decided:** Used info.title || null (not ?? null) so empty titles from session.created become null — the curator's last-writer-wins-only-when-Some guard then skips them, avoiding a transient empty-string title
- **Decided:** Combined session.created and session.updated into one if-block (same payload shape, same action) rather than two separate handlers
- **Decided:** model flattened to providerID/modelID only when both fields are truthy; otherwise null — avoids emitting a partial 'providerID/' or '/modelID' string
- **Decided:** Used typed casts (as { agent?: string }) instead of bare `as any` — typecheck passes and is more precise than the plan's sketch

## [0b0aa5b] Slice 6a live QA complete — hardening plan saved for handoff

Slice 6a (SessionUpdated collection layer) is shipped and live-verified. Three commits on feat/curator-activity-item: schema variant (7f4bac5), curator ingest (1acf52f), plugin handlers (8555572 in cue-plugins). The pipeline works end-to-end — lineage, agent, and early title all confirmed via live QA against a fresh acuity DB.

Live QA + a deep opencode source investigation (doc at .cue/feat-curator-activity-item/doc/1782664752-0b0aa5b/opencode-model-capture.md) revealed two problems that need hardening before Slice 6b rendering:

1. DUPLICATION: opencode fires session.created + multiple session.updated for the same state. The handler emits a SessionUpdated for each with no dedup. Fresh DB showed 5 identical session_updated rows for one session (59% noise in an earlier dump). Fix: persistent lastSessionSig Map in the plugin that skips identical consecutive events.

2. MODEL GAP: model is always null. Root cause is architectural, not a bug — Task-tool subagent sessions are created with no model (task.ts:129-145), and the model is resolved per-turn at runtime. The session record never carries it for subagents. The reliable source is the AssistantMessage's typed modelID/providerID fields, accessible in the message.updated handler we already have. Fix: add model: Option<String> to AgentTurnCompleted and populate from the assistant message.

Version mismatch identified: opencode runtime is 1.17.11, but cue-plugins package.json specified "*" which resolved to SDK 1.17.9. User has updated the runtime to 1.17.11 and requested pinning the cue-plugins deps to match.

A full executive plan with 7 steps (5 commits across 2 repos) is saved at .cue/feat-curator-activity-item/plan/1782664752-0b0aa5b/slice6a-hardening.md. Handing off to another agent to execute.

- **Found:** Live QA confirmed the 6a pipeline works: parent_id lineage captures correctly for sub-agent sessions, agent captures correctly (plan vs general), title arrives via session_updated BEFORE session_idle (the core 6a goal)
- **Found:** session_updated events are severely duplicated: opencode fires session.created + multiple session.updated for the same state. Fresh DB: 5 identical rows for 1 session. The handler has no dedup unlike the turn/call handlers which use a dedup Map
- **Found:** model is always null on SessionUpdated — this is architectural, not a bug. Task-tool subagent sessions are created with no model; the model is resolved per-turn at runtime via a fallback chain (session.model -> catalog default -> first supported) and is only visible in turn-level events
- **Found:** AssistantMessage in SDK 1.17.9 (and 1.17.11) has modelID and providerID as required typed fields (types.gen.d.ts:108-109). We already handle message.updated for assistant messages — model can be captured there with typed access, no cast needed
- **Found:** session.next.step.started event exists in the ref source and likely in SDK 1.17.11, carrying the resolved model at turn start. But unnecessary for stage B — message.updated already provides it at turn completion
- **Found:** opencode runtime updated to 1.17.11; installed SDK was 1.17.9 (package.json spec was *). The ref source types agent?/model? on Session, so pinning to 1.17.11 should allow dropping the defensive casts in the SessionUpdated handler
- **Found:** client.session.get() returns the same projection as the event payload — cannot recover model for subagent sessions
- **Found:** Native agents plan/general define no model in their config (agent.ts:143-179) — GET /agent only returns models for user-configured agents
- **Decided:** Model capture goes on AgentTurnCompleted (not SessionUpdated) — the model is per-turn and AgentTurnCompleted fires from message.updated which carries typed modelID/providerID on the assistant message
- **Decided:** SessionUpdated.model is kept (not removed) — cooperates with AgentTurnCompleted.model via curator last-writer-wins, occasionally useful for explicit-model sessions
- **Decided:** Dedup uses a persistent lastSessionSig Map separate from the turn/call dedup Map (which is cleared on idle) — collapses identical consecutive session events across idle boundaries
- **Decided:** No session.next.step.started handler — message.updated already provides the model at turn completion which is when the activity feed displays it
- **Decided:** Pin @opencode-ai/sdk and @opencode-ai/plugin to 1.17.11 to match the runtime
- **Open:** After pinning to 1.17.11, verify whether the SDK Session type now declares agent?/model? — if so, drop the defensive casts in the SessionUpdated handler (plan step 2)
- **Open:** Verify the 5-commit hardening plan produces clean dedup and non-null model via live QA against the running acuity instance at .cue/feat-curator-activity-item/tmp/acuity-phase-6a

## [369a0d1] Phase A shipped — acuity per-variant logging + optional file output

Commit 369a0d1 on feat/curator-activity-item. Refactored the tracing subscriber to dual-layer (stderr + optional file). Per-variant INFO fields replace the opaque single info! at the ingest site. DEBUG line adds raw+parsed delta. 39/39 acuity tests pass, clippy clean.

- **Found:** Debug derive already present on all AcuityEvent variants — A1 was a no-op
- **Found:** Mutex<BufWriter<File>> compiles cleanly as MakeWriter in tracing-subscriber 0.3 with the existing feature set
- **Found:** The per-layer .with_filter() API works correctly with Option<Layer> in the registry composition
- **Found:** AcuityEvent local alias (use acuity_schema::AcuityEvent as Ev) inside the match avoids repeating the full path per arm without adding an import at the module level
- **Decided:** Truncate-on-start for the file layer (write+truncate, not append) — each cargo run is a clean experiment
- **Decided:** Dual-filter: stderr at RUST_LOG/acuity=info, file at ACUITY_LOG_LEVEL/acuity=debug — terminal stays quiet, file is rich
- **Decided:** ACUITY_LOG_FILE env var with no default path — user sets it explicitly to a path inside the workspace mount
- **Decided:** model = ?e.model commented out from agent_turn_completed arm — will be uncommented in B3 after the field is added to the schema

## [abf7772] 369a0d1 + abf7772 — Phase A and schema B3 shipped; plugin work not yet started

Two commits on feat/curator-activity-item:\n\n1. 369a0d1 feat(acuity): structured per-variant logging + optional file output\n   - Dual-layer tracing subscriber: stderr at RUST_LOG/acuity=info; optional file at ACUITY_LOG_LEVEL/acuity=debug.\n   - ACUITY_LOG_FILE env var enables file layer (truncate-on-start, no default path).\n   - Per-variant INFO match replaces single opaque info! at the ingest site.\n   - debug! line with %payload + ?event for raw-vs-parsed triage.\n   - 39/39 acuity tests pass, clippy clean.\n\n2. abf7772 feat(acuity-schema): add model to AgentTurnCompleted\n   - pub model: Option<String> added after output_tokens.\n   - Test helper and raw-JSON deserialize test updated (TDD RED-GREEN).\n   - 25/25 acuity-schema tests pass, clippy clean.\n\nPlugin work (B1 SDK pin, B2 dedup, B4 model capture) not started. Two B3 follow-up tasks also outstanding: regenerate types.ts and add model = ?e.model to the log arm in main.rs.",

- **Found:** Debug derive was already present on all AcuityEvent variants — A1 was a no-op
- **Found:** Mutex<BufWriter<File>> works cleanly as MakeWriter in tracing-subscriber 0.3 with the existing feature set, no new dependency needed
- **Found:** Per-layer .with_filter() composes correctly with Option<Layer> in the registry
- **Found:** The AgentTurnCompleted log arm in 369a0d1 omits model because the field did not exist yet in the schema at that point — the arm needs a follow-up edit after abf7772
- **Decided:** Truncate-on-start for file layer: each cargo run is a clean experiment
- **Decided:** Dual-filter: stderr INFO for human, file DEBUG for agent analysis
- **Decided:** ACUITY_LOG_FILE with no default: user sets explicitly to a workspace-mounted path
- **Decided:** Curator model ingest deferred until plugin B1-B4 is live-verified via logs
- **Open:** B3 follow-up: regenerate types.ts (codegen binary)
- **Open:** B3 follow-up: add model = ?e.model to AgentTurnCompleted arm in main.rs
- **Open:** B1: pin SDK to 1.17.11 and verify Session type gains agent?/model? fields
- **Open:** B2: add lastSessionSig dedup Map to plugin session handler
- **Open:** B4: add model field to AgentTurnCompleted payload in message.updated handler
- **Open:** Live verification: run acuity with ACUITY_LOG_FILE set and observe per-variant log output

## [f5c1695] Phase B shipped — plugin SDK pin, dedup, model capture; acuity log arm complete

Four commits across two repos complete Phase B of slice6a-logging-and-plugin.md:

Cue repo (1 commit):
1. f5c1695 feat(acuity): log model field on AgentTurnCompleted
   - Added `model = ?e.model` to the AgentTurnCompleted INFO log arm (omitted in 369a0d1 because the field didn't exist yet; abf7772 added it to the schema).
   - Updated the insert_agent_turn_completed_turn_id_populated test helper in db.rs.
   - 39/39 acuity tests pass, clippy clean.

Cue-plugins repo (3 commits):
2. f3b11f7 chore(acuity-plugin): pin opencode sdk/plugin to 1.17.11
   - Pinned @opencode-ai/sdk and @opencode-ai/plugin from '*' (resolved 1.17.9) to 1.17.11.
   - SDK 1.17.11 Session type STILL omits agent?/model? — defensive casts stay.
   - Fixed the model cast field name: modelID → id (Session.model runtime shape is { id, providerID }, not { modelID, providerID }). This was a latent bug in the SessionUpdated handler.
3. db90d51 fix(acuity-plugin): dedup session_updated by metadata signature
   - Persistent lastSessionSig Map keyed by sessionID, signature over [parent_id, agent, model, title].
   - Collapses identical consecutive session.created/session.updated events (was producing 5+ identical rows per session).
   - NOT cleared on idle (unlike the turn/call dedup Map) — duplicates span idle boundaries.
4. 3aeb835 feat(acuity-plugin): capture resolved model from assistant messages
   - Added `model: \`${info.providerID}/${info.modelID}\`` to AgentTurnCompleted payload in the message.updated handler.
   - AssistantMessage has providerID/modelID as required typed fields — no cast, no null guard.
   - Regenerated types.ts (B3 follow-up) to include the model field.

All commits typecheck clean (tsc --noEmit). Live QA pending: user needs to restart acuity with the new code and trigger an opencode session to verify dedup + model capture via the log file.

- **Found:** SDK 1.17.11 Session type STILL does not declare agent?/model? — the defensive casts in the SessionUpdated handler must stay
- **Found:** Session.model runtime shape is { id, providerID, variant? }, NOT { modelID, providerID } — the previous cast used modelID which was a latent bug (would always produce null model on SessionUpdated even for explicit-model sessions)
- **Found:** AssistantMessage in SDK 1.17.11 has modelID: string and providerID: string as required typed fields (types.gen.d.ts:108-109) — no cast needed for B4
- **Found:** git add -p with `s` (split) cleanly separates adjacent hunks that share context lines — used to split the B1b modelID→id payload fix from the B2 dedup logic in the same handler block
- **Found:** types.ts regen and B4 model capture are tightly coupled — the regen makes model a required field on AgentTurnCompleted, so the plugin payload must supply it in the same commit or typecheck fails
- **Decided:** Combined B1b into the B1 commit (f3b11f7) rather than a separate commit — both are SDK-version-driven changes to the same handler
- **Decided:** Fixed the Session.model cast field name (modelID → id) as part of B1b even though SDK doesn't type the field — the plan's investigation established the runtime shape and the old cast was wrong
- **Decided:** Committed types.ts regen + B4 together (3aeb835) — the regen creates the type requirement, B4 satisfies it; splitting would leave a non-typechecking intermediate state
- **Decided:** Kept SessionUpdated.model rather than removing it — cooperates with AgentTurnCompleted.model via curator last-writer-wins, useful for explicit-model sessions
- **Open:** Live QA: restart acuity with new code, trigger opencode session, verify dedup (session_updated count drops from 5+ to 1-2) and model capture (agent_turn_completed shows model=Some(...)) via log file
- **Open:** Curator ingest of model from AgentTurnCompleted (hardening plan step 6) — deferred until Phase B is live-verified
- **Open:** If 1.17.11 SDK later adds agent?/model? to Session type, the casts in SessionUpdated handler can be simplified

## [d590d61] Slice 6a hardening complete — curator model ingest shipped, all steps done

Commit d590d61 on feat/curator-activity-item. Final step of the slice6a-hardening plan (step 6, TDD).

Added last-writer-wins model ingest to the agent_turn_completed match arm in curator's push_event (app.rs:255-259). SessionSummary.model is now populated from both event sources:
- SessionUpdated (session-level, null for subagent sessions at creation)
- AgentTurnCompleted (turn-level, always carries the resolved model from AssistantMessage.providerID/modelID)

TDD cycle: RED confirmed (model left as None before the fix), GREEN confirmed (model set from ev.model). 48/48 curator tests pass. Full workspace: 304 tests pass, clippy clean across all crates.

Updated test helpers: turn() in app.rs now includes model: Some("anthropic/claude-sonnet"), activity.rs and ui.rs helpers use model: None (they test layout/summary, not model ingest).

This completes the entire slice6a-hardening plan. All 7 steps are now shipped:
1. f3b11f7 (cue-plugins) — SDK pin to 1.17.11 + cast field name fix
2. db90d51 (cue-plugins) — session_updated dedup
3. f5c1695 (cue) — acuity log arm for model field
4. abf7772 (cue, earlier) — schema model field on AgentTurnCompleted
5. 3aeb835 (cue-plugins) — model capture from assistant messages
6. d590d61 (cue) — curator ingest of model

Live QA confirmed all fixes working: dedup collapsed session_updated from 5+ to 2 per session, model captures correctly on every agent_turn_completed (including subagents with google/gemini-3-flash-preview), wire-vs-deser delta clean.

- **Found:** Three other test helpers (activity.rs x2, ui.rs x1) construct AgentTurnCompleted and needed model: None added — the schema field added in abf7772 is required, not optional in the struct literal
- **Found:** The existing session_updated model handler (app.rs:242-243) already uses the same if-let-Some pattern, so the agent_turn_completed handler mirrors it exactly for consistency
- **Decided:** Model ingest uses last-writer-wins (same as session_updated handler) — AgentTurnCompleted.model and SessionUpdated.model cooperate, whichever fires last with Some wins
- **Decided:** turn() test helper gets model: Some(...) (realistic value for model-ingest tests), while activity.rs and ui.rs helpers get model: None (they test layout/summary formatting, not model ingest)

## [d590d61] State-of-branch snapshot for handoff — spec/index.md created, all plans consistent

All Slice 6a work (collection + logging + hardening) is shipped, live-verified, and marked complete. The branch is ready for Slice 6b (rendering).

Created spec/index.md as the anchor document for this branch — a fresh agent should read that first for the complete picture.

Git state:
- cue repo (feat/curator-activity-item): HEAD at d590d61, 2 commits ahead of origin
- cue-plugins repo (master): HEAD at 3aeb835

All plan statuses verified consistent:
- slice5-activity-item: complete
- slice6-review-fixes: complete
- slice6a-collection: complete
- slice6a-hardening: complete
- slice6a-logging-and-plugin: complete
- slice6b-rendering: open (the next step)

Next work item: Slice 6b rendering (plan/1782659497-0c2ff37/slice6b-rendering.md). The plan is intentionally light — it needs fleshing out now that 6a's live data is in hand. Key goals: kill the 8-char session-id truncation (ui.rs:214), surface title+agent+model+lineage in headers, add dev debug columns. Retains flat reverse-chrono layout; nested tree is stage C.

- **Found:** spec/index.md did not exist for this branch — the only spec artifact was log.md. A fresh agent following the cue skill would look for spec/index.md first and find nothing
- **Found:** slice6a-collection.md was still marked status: open despite being fully shipped — corrected to complete
- **Found:** All 6 plans now have consistent statuses: 5 complete, 1 open (slice6b-rendering, the next step)
- **Decided:** Created spec/index.md as the branch anchor document — it was missing and a fresh agent had no single document to understand the branch intent and current state
- **Decided:** Marked slice6a-collection.md as complete — it was still open despite all steps being shipped and superseded by the hardening plan

## [d590d61] acumen design discussion concluded, artifacts written

Extended design discussion for the `acumen` graph index crate. Two external consultants (Opus, GLM) reviewed the design and surfaced critical gaps. The design was revised and finalized in response.

- **Found:** refs: field does not exist anywhere in the current corpus -- relationships are in prose ## Source sections only
- **Found:** acuity is telemetry-only today; cannot drive corpus sync without a new CorpusChanged event type
- **Found:** neo4rs is not present in any Cargo.toml; Neo4j was not as established a dependency as assumed
- **Found:** COLOCATED edges would be O(n^2) per dir with low signal value -- dropped
- **Found:** Non-markdown artifacts (json, sh, log) have no frontmatter -- all frontmatter-derived node fields must be nullable
- **Found:** The .cue/ worktree is an orphan branch of the same repo, not a separate repository
- **Decided:** Use SQLite as the graph backend (embedded, daemon-free, .cue/.acumen/graph.db, gitignored)
- **Decided:** Graph DB is a materialized view -- fully deterministically reconstructable from committed .cue/ markdown files alone
- **Decided:** refs: as a flat list of paths in frontmatter -- no typed relationship syntax
- **Decided:** git_branch: field in spec/index.md only, written by branch-init shell script
- **Decided:** No updated: frontmatter field -- source updated_at from git log / fs mtime
- **Decided:** PRECEDES edges only for plan/ and trace/ types where sequence is meaningful
- **Decided:** COLOCATED edges dropped entirely
- **Decided:** Full rebuild sync strategy (idempotent); manual trigger for MVP
- **Decided:** GraphBackend trait keeps Neo4j as a future option
- **Decided:** acuity CorpusChanged event is a valid future evolution target, not MVP
- **Decided:** Gardening delegated to LLM agents, not programmatic for now
- **Decided:** Semantic search deferred
- **Decided:** Handoff skill: new concept, runs at session end, ensures spec/index.md and refs: are complete
- **Open:** Phase 0 (refs: in RawFrontmatter + branch-init script) has not been started
- **Open:** Handoff skill needs to be written in cue-plugins
- **Open:** acumen crate does not exist in the workspace yet

## [0521339] Slice 6b design locked — lean headers, hide session_updated, defer column collapse

Design session for Slice 6b (curator activity feed rendering), informed by a fresh read of ui.rs/app.rs/activity.rs and an Opus consultation. The original draft plan (surfaces title+agent+lineage in headers, adds dev debug columns) was revised substantially based on user feedback. The revised plan is written to plan/1782659497-0c2ff37/slice6b-rendering.md.

Pipeline recap: opencode-plugin -> acuity server (SQLite) -> curator TUI (SSE). Slice 6a (collection) is shipped and live-verified: SessionSummary carries title, agent, model, parent_id, harness. Slice 6b is the rendering layer that makes that data legible in the flat reverse-chrono activity feed. Stage C (nested parent/child tree) is out of scope.

- **Found:** render_diagnostics (ui.rs:241-311) already solves selection-over-filtered-set: filters tool_call_*, sel_diagnostics indexes the filtered Vec, render-time .min(len-1) clamp (ui.rs:306). This is the template for hiding session_updated rows — mirror it, do not invent.
- **Found:** scroll_down_activity clamps against events.len() (app.rs:360) but the visual list is longer (headers are injected) — a latent bug the filtered-set rework fixes for free.
- **Found:** Skip-on-render (keep sel_activity indexing all events, skip session_updated in render) causes the highlight to vanish/jitter when sel lands on a hidden index — rejected approach.
- **Found:** session_updated rows are pure noise: push_event absorbs their payload into SessionSummary synchronously (app.rs:230-250) before render, so the session header already shows their content by the time the row would appear on screen.
- **Found:** .get(..8) at ui.rs:214 takes the first 8 chars (prefix); opencode ids share a per-run prefix (ses_0f14...) so 5 distinct sessions render identically. The unique part is the suffix.
- **Found:** The title-flip (dim id placeholder -> bright title when session_updated lands) IS the live verification that 6a title-capture round-trips to the UI — design its visual distinction deliberately rather than as a silent fallback.
- **Found:** event_summary for agent_turn_completed (ui.rs:333-343) deserializes AgentTurnCompleted which already carries model: Option<String> — appending the per-turn model is a one-line change in that arm.
- **Found:** Two test-helper copies of make_record exist (ui.rs:390-401 and activity.rs) — reuse the ui.rs scaffold for new pure helpers, do not add a third.
- **Decided:** Header = title | dim id-suffix placeholder only. No agent (belongs in diagnostics — plan-agent-mutating-files is the signal), no session-level model (misleading fiction — models change per turn), no parent-id text (stage-C tree shows lineage visually).
- **Decided:** Per-turn model on turn rows from AgentTurnCompleted.model; appended as ' · {model}' when Some, omitted cleanly (no dangling separator) when None.
- **Decided:** Harness + project in the block title once ('Activity Feed · opencode / cue'), not per header — they are workspace-constant today; a pure helper (activity_block_title) makes per-header a one-line change if a second harness ('pi') lands.
- **Decided:** Hide session_updated rows entirely; selection indexes the filtered set, mirroring Diagnostics: new is_hidden_in_activity() + App::activity_len() + render-time .min() clamp + empty-list guard.
- **Decided:** session_label pure helper returns (String, is_placeholder: bool); header style conditional — DarkGray when placeholder, brighter/bold when title present so the flip is visible.
- **Decided:** Leave the 24-char event_type column as-is; collapse deferred to observe in practice and make a more informed call later.
- **Decided:** Pure helpers first (TDD red/green, no frame), wiring last; fix scroll_down_activity clamp (events.len() -> activity_len()) BEFORE wiring — it is the correctness keystone.
- **Decided:** build_activity_items stays unwired; its (session_id, project_dir) header key is degenerate (project_dir workspace-constant); stage C rewrites with parent_id-based nesting. Add doc-comment only.
- **Decided:** Acuity server log trim -> standalone todo, NOT this slice (6b is curator TUI only; the log is a separate concern).
- **Open:** Stage C: nested tree with parent_id-based nesting, folding child sessions under parent turns.
- **Open:** event_type column collapse — deferred to observe the current rendering in practice first.
- **Open:** id-suffix collision risk (two sessions sharing a suffix) — noted, accepted for stage B; stage C keys by full session_id/parent_id.
- **Open:** Render-time clamp behavior on live ring-buffer eviction (selected event ages out) — covered defensively by the .min() guard copied from Diagnostics.

## [81f1853] Step 1 — session_label helper shipped (commit 81f1853)

- **Found:** session_label suffix uses last ~8 chars via boundary-guarded str::get with unwrap_or fallback — session ids are ASCII in practice so exact
- **Found:** Test expectation error caught during GREEN: 'ses_0f14abc123' is 14 chars, last 8 = '14abc123' (not 'f14abc123') — fixed test, implementation was correct
- **Found:** Function is dead-code until step 6 wires it into render_activity — added #[allow(dead_code)] with a wiring note
- **Decided:** session_label signature: (Option<&SessionSummary>, &str) -> (String, bool) — matches plan, clean at render call site
- **Decided:** Empty title string treated as no title (mirrors plugin's info.title || null guard) — falls through to id-suffix placeholder

## [9295f21] Step 2 — per-turn model in event_summary shipped (commit 9295f21)

- **Found:** match on ev.model (Option<String>) is the cleanest form — no format! with conditional fragments, no dangling separator
- **Decided:** Used middle-dot (U+00B7) '·' as separator — same glyph already used elsewhere in the UI (e.g. ERR em-dash U+2014); keeps the turn row compact

## [d5ae36c] Step 3 — is_hidden_in_activity + activity_len shipped (commit d5ae36c)

- **Found:** Both helpers register as dead-code until step 6 wires them into render_activity and scroll_down_activity — added #[allow(dead_code)] with wiring notes (same pattern as session_label in step 1)
- **Decided:** activity_len delegates to ui::is_hidden_in_activity for the predicate — single source of truth for the hide rule, avoids duplicating the 'session_updated' string literal in app.rs

## [9b9be0e] Step 4 — scroll_down_activity clamp fixed (commit 9b9be0e)

- **Found:** The latent bug was pre-existing: scroll_down_activity clamped against events.len() which was already longer than the visual list (headers are injected). The filtered-set fix resolves this for free — activity_len counts visible events, matching the rendered row count
- **Decided:** Removed #[allow(dead_code)] from activity_len — it is now genuinely called by scroll_down_activity, so the allow is no longer needed
- **Decided:** Kept is_hidden_in_activity and session_label allows — they remain dead until step 6 wiring

## [4dfef13] Step 5 — activity_block_title shipped (commit 4dfef13)

- **Decided:** activity_block_title reads the first available SessionSummary via sessions.values().next() — harness/project are workspace-constant today, so any session's values are representative
- **Decided:** project basename via rsplit('/').next() with empty-guard filter, mirroring the old session_header logic

## [cfa450e] Steps 6+8 — render_activity rebuilt + activity.rs doc shipped (commit cfa450e)

- **Found:** Removed all 3 now-redundant #[allow(dead_code)] annotations on session_label, is_hidden_in_activity, activity_block_title — they are now genuinely called by render_activity
- **Found:** visible.iter().enumerate() yields (usize, &&EventRecord) — autoderef handles record.session_id access cleanly, no explicit deref needed
- **Decided:** Header style: DarkGray for placeholder, Cyan+Bold for titled — matches the kanban active-column emphasis color for visual consistency
- **Decided:** prev_session tracked as Option<&str> (no clone) — borrows from app.events through the visible Vec; lifetime-safe across iterations
- **Decided:** sel computed once before the loop (clamped to visible.len()-1 or None when empty) — mirrors render_diagnostics ui.rs:305-306
- **Decided:** Folded step 8 (activity.rs doc-comment) into this commit — both are the final slice-6b non-test changes

## [cfa450e] Slice 6b code-complete — 6 commits, 60 curator tests green, manual QA pending

- **Found:** Full workspace: 22 test suites all green, clippy clean across all crates. Curator crate went from 48 to 60 tests (+12 new tests across the 6 commits)
- **Decided:** Plan status set to in-progress (not complete) — steps 1-6 and 8 are code-complete and committed; step 7 (manual live QA) is pending user verification
- **Open:** Step 7 manual live QA: (a) prefix-sharing sessions render distinct headers; (b) header flips dim-id to bright-title within seconds; (c) session_updated rows gone; (d) j/k never makes highlight vanish/jitter; (e) turn rows show per-turn model; (f) block title shows harness/project once

## [cfa450e] Master task board reconciled — activity-item-model complete, view-redesign split

Reconciled the two master tasks that mapped to feat/curator-activity-item, which had both been left at status: open with empty Evidence cells throughout the branch's lifetime.

Four changes:
1. activity-item-model.md -> status: complete. All 11 Evidence cells filled (criteria 1-2 code review cites activity.rs:17-31/75; criteria 3-10 cite the specific unit test names; criterion 11 cites 60 passed). This task tracked Slice 5 (ActivityItem + build_activity_items) which is fully shipped.
2. activity-view-redesign.md -> status: closed (superseded). Added a divergence note explaining the original full-Slice-6 vision was split: rendering legibility shipped as Slice 6b (with redesigned header format and selection model), fold-state/toggle deferred to stage C.
3. NEW activity-feed-rendering.md -> status: in-progress, branch: feat-curator-activity-item. Accurately describes the Slice 6b rendering work that shipped (6 commits, 7 of 8 criteria met; criterion 8 manual QA pending).
4. NEW activity-fold-state.md -> status: open, priority: low. Captures the deferred fold-state/toggle/nested-tree work (stage C), split out from the closed view-redesign task.

The board now reflects reality: one complete task, one in-progress (the active branch work), one open future task, and one closed (superseded).

- **Found:** Neither original task had been transitioned to in-progress with branch: set during the branch's lifetime — a bookkeeping gap from earlier sessions
- **Found:** activity-view-redesign.md criteria 1,3,4,5,7 all diverged or deferred from the original Slice 6 vision; only criteria 8 (no-drift) and 9 (tests pass) were met by the redesigned work
- **Decided:** Option B: closed activity-view-redesign with divergence note, created two new tasks instead of keeping a mismatched card in-progress
- **Decided:** activity-fold-state.md is priority: low — no fixed trigger, actioned when the flat layout becomes a bottleneck or persistent sessions land

## [cfa450e] Slice 6c design locked — two-pane activity view

Extended design session for the curator Activity view redesign. Two UX defects
from Slice 6b manual QA drove the design: (1) harness/project info appeared in
the pane border title rather than on the session row, (2) flat reverse-chrono
rendering caused events from concurrent sessions to interleave and sessions to
jump position on every new event arrival.

Proposed and rejected: a single-pane grouped layout (group events by session,
sort groups by first_seen). Rejected because index-based selection becomes
materially worse under grouping (a new session inserts a whole group,
shifting every existing index), and because activity.rs already provides a more
sophisticated grouping engine (build_activity_items with SessionHeader/Turn/
Standalone) that would become a competing parallel implementation.

Adopted: two-pane master-detail layout.
- Pane 1 (Sessions, left 1/3): list of sessions sorted by first_seen desc
- Pane 2 (Events, right 2/3): events for the selected session, flat reverse-chrono

Consulted Opus twice: once on the single-pane grouping plan (flagged parallel
engine + selection drift), once on the two-pane design (confirmed sound, flagged
input wiring gaps + sel_activity reset invariant + auto-selection boundary).

Plan: .cue/feat-curator-activity-item/plan/1783746571-cfa450e/slice6c-two-pane-activity.md

- **Found:** activity.rs:build_activity_items is turn-folding scaffolding designed for stage C (parent_id nesting + turn expand/collapse). It is NOT a grouping engine for flat session lists. Stage-B Pane 2 uses a simpler per-session flat filter; stage-C wires build_activity_items into Pane 2 with a session_id filter parameter.
- **Found:** Index-based selection is materially worse under first_seen-sorted grouping than the current flat list: a new session inserts a whole group, shifting every event index. Opus flagged this as critical.
- **Found:** app.sessions HashMap never shrinks (push_event only inserts, never removes). Sessions with all events evicted from the ring buffer still appear in the HashMap. sorted_sessions() must filter by session_event_len > 0 to avoid empty Pane 2.
- **Found:** first_seen and session_id on SessionSummary are both #[allow(dead_code)] today. Both are read by the new two-pane logic — dead_code annotations must be removed.
- **Found:** auto-selection of the first session must NOT live in push_event (keeps the data-ingest method pure). It belongs in ensure_session_selection() called from process_msg after push_event.
- **Found:** Action derives Copy in event.rs. New action variants must be unit variants (no String payloads) to preserve Copy.
- **Found:** status_help_line is shared between Activity and Diagnostics views. A new activity_help_line is needed to add Tab/z hints without polluting the Diagnostics help bar.
- **Found:** The event ring buffer evicts individual events (oldest first, cap 2000). This can leave a session in app.sessions with zero visible events. The user's preferred future model is per-session eviction (evict oldest complete session).
- **Decided:** Two-pane layout: Sessions (Ratio 1/3, left) + Events (Ratio 2/3, right).
- **Decided:** Session selection is identity-based: sel_session_id: Option<String>. Cursor follows the session, not the visual index. New sessions prepend at top; existing selection is unaffected.
- **Decided:** Event selection stays index-based: sel_activity: usize. Resets to 0 on session change. Shifts are within one session only — acceptable for stage B.
- **Decided:** Invariant: any mutation of sel_session_id resets sel_activity = 0. Enforced in scroll_down/up_sessions and ensure_session_selection.
- **Decided:** z key toggles active pane to fullwidth; Tab / Shift-Tab switches panes (stays expanded when expanded).
- **Decided:** sorted_sessions() filters to sessions with visible events AND sorts first_seen desc with session_id asc tiebreak. Recomputed fresh each call — no caching (n < 20, O(n log n) negligible).
- **Decided:** activity.rs is untouched. No competing parallel grouping engine. Stage-C Pane 2 uses build_activity_items with a session_id filter.
- **Decided:** Colors: project=Magenta, harness=Blue, title=Cyan+Bold (titled) / DarkGray (placeholder). Both Magenta and Blue are currently unused in ui.rs.
- **Decided:** activity_block_title() deleted. Block titles are per-pane: Sessions / Events · {title}.
- **Decided:** session_label() unchanged (reused for both session row label and events pane block title).
- **Decided:** Two deferred todos created: improve-session-data-loading (per-session ring-buffer eviction + startup N-session fetch), collect-user-turn-messages (human prompt events).

## [cfa450e] Slice 6b manual QA complete — activity-feed-rendering task closed

User confirmed all six Slice 6b visual QA criteria manually on 2026-07-11. Evidence cell filled in activity-feed-rendering.md; task transitioned to complete. Ready to begin Slice 6c two-pane implementation.

- **Found:** All six Slice 6b live QA criteria confirmed: distinct headers for prefix-sharing sessions, header flip dim-id->bright-title, session_updated rows hidden, j/k selection stable, per-turn model on turn rows, harness/project block title
- **Decided:** Transition activity-feed-rendering.md task to complete with user attestation as evidence for criterion 8

## [1ce0a99] Slice 6c shipped — two-pane activity view (commit 1ce0a99)

Full two-pane activity view implemented in a single commit on feat/curator-activity-item. Curator crate went from 60 to 78 tests (+18 net: +16 app, +4 ui, -2 deleted activity_block_title tests).

- **Found:** project_basename trailing-slash edge case: rsplit('/').next() on '/home/pl/code/' yields empty string, filter(!empty) rejects it, unwrap_or falls back to full path. Test expectation corrected.
- **Found:** scroll_down_activity needed sel_session_id set in the existing scroll-down test — updated the test to set sel_session_id before scrolling
- **Found:** map_or(false, ...) flagged by clippy; replaced with is_some_and
- **Found:** ActivityPane::Events variant triggered dead_code warning until step 2 wiring — resolved automatically after main.rs wiring
- **Decided:** All 4 steps (app state + helpers, input/event/main wiring, render rework, activity.rs doc-comment) committed as a single atomic commit — changes are tightly coupled and the intermediate states would not compile or render correctly
- **Decided:** Deferred todos (improve-session-data-loading, collect-user-turn-messages) created as point-in-time todo artifacts per the plan
- **Decided:** project_basename trailing-slash behavior documented in test comment (falls back to full path, not to parent dir)
- **Decided:** activity_len kept with #[allow(dead_code)] for its 2 existing tests; session_event_len is the per-session equivalent used in production
- **Open:** Step 6 manual live QA: events grouped by session, stable session list, Tab/z pane switching, per-session j/k, color-coded session rows, Events block title, empty-events placeholder, Diagnostics unaffected

## [1ce0a99] Slice 6d design locked — gitui-style UX + columnar session rows

Design session for Slice 6d (curator activity view UX redesign), informed by gitui screenshots and a full read of ui.rs/app.rs/event.rs/input.rs/main.rs at commit 1ce0a99.

The two primary changes are:
1. Columnar fixed-width session rows in the Sessions pane (project | datetime | harness-abbrev | title)
2. gitui-inspired three-state layout (SessionsFull → Split → DetailFull) replacing the current Tab/z pane model

- **Found:** chrono is not in Cargo.toml — must be added as a new dep (features=[clock] for Local::now())
- **Found:** active_activity_pane + pane_expanded are referenced in 35 places across 5 files (app.rs, main.rs, ui.rs, event.rs, input.rs) — all must be updated atomically in commit 3
- **Found:** Two tests will be replaced: switch_activity_pane_toggles and toggle_pane_expand_toggles → new tests for toggle_detail_pane, enter_detail_full, return_from_detail_full
- **Found:** Left/Right in process_msg are currently NOT view-gated — they always call move_left()/move_right() regardless of active_view. Commit 3 must gate them: Left is kanban-only, Right is view-aware (kanban: move_right, Activity: enter_detail_full)
- **Found:** SessionSummary.last_seen is the correct field for the datetime column (updated on every event — represents most recent activity)
- **Found:** session_label function is reused for the Events block title in DetailFull (session context visible even without sessions pane)
- **Decided:** Columnar session rows: 4 columns — project (Magenta, 8 wide), datetime (Cyan, 12 wide, last_seen in local TZ), harness-abbrev (Blue, 2 wide: oc/cc/pi/??), title (fill, Cyan+BOLD / DarkGray placeholder). datetime is Cyan not DarkGray — DarkGray matches the selected row highlight bg and makes values invisible when selected.
- **Decided:** Datetime format: HH:MM for today (5 chars), Mmm DD HH:MM for other days (12 chars), padded to 12 with format!. Requires chrono 0.4 with features=[clock] as a new Cargo dep.
- **Decided:** harness_abbrev pure function: opencode→oc, claudecode→cc, pi→pi, else ??
- **Decided:** Three-state ActivityLayout enum replaces ActivityPane enum + pane_expanded bool: SessionsFull (default fullscreen sessions list), Split (sessions left focused + detail right static), DetailFull (detail/events fullscreen navigable)
- **Decided:** Navigation: Enter toggles SessionsFull↔Split; Right arrow from SessionsFull or Split → DetailFull; Escape from DetailFull → Split. Left arrow does NOT navigate back from DetailFull (user spec, differs from gitui). Left is no-op in Activity view.
- **Decided:** Tab/SwitchPane and z/ToggleExpand actions removed entirely from event.rs and input.rs.
- **Decided:** Action::Enter (KeyCode::Enter) and Action::Escape (KeyCode::Esc) added to event.rs and input.rs.
- **Decided:** Detail pane structure: vertically split into Info block (top, Constraint::Length(8), always static) + Events list (remaining, navigable only in DetailFull). Info shows: title, agent, model, parent_id, tokens in/out, error_count — all from SessionSummary.
- **Decided:** In Split mode, sessions pane is focused (j/k navigates sessions); detail pane is static (dim border, dim events). In DetailFull, j/k navigates events with active highlight.
- **Decided:** activity_help_line is layout-aware: Split/SessionsFull shows Enter/→ hints; DetailFull shows Esc hint.
- **Decided:** render_sessions_pane is always called with is_active=true (it is only rendered when sessions pane is the focused element).
- **Decided:** Event tree folding is out of scope — flat events list retained for this slice (stage C).
- **Decided:** Commit plan: 4 commits — (1) helpers (chrono dep + format_datetime + harness_abbrev, TDD), (2) columnar sessions render, (3) ActivityLayout refactor (app+event+input+main+ui dispatch, TDD), (4) detail pane Info section + full render redesign.
- **Open:** Slice 6d manual live QA after commit 4

## [27dceba-dirty] Slice 6d partial — steps 1-3 committed, step 4 code-complete but unverified

Implementing slice6d-gitui-ux.md (4-commit plan). Steps 1-3 are committed. Step 4 code is written but cargo check/test could not be verified due to a transient system resource exhaustion (rustc panics with EAGAIN on ctrlc handler install — OS signal handler limit hit). The system was similarly failing for the acuity integration tests (27 failures visible in workspace test run that are pre-existing).

Commits on feat/curator-activity-item:
1. 84908d6 feat(curator): add format_datetime + harness_abbrev helpers
2. 7ef8690 feat(curator): columnar session rows with local datetime + harness abbrev
3. 27dceba refactor(curator): replace ActivityPane+pane_expanded with ActivityLayout
4. Step 4 — code written but NOT committed (render_detail_pane + render_session_info + Info block)

Step 4 changes applied to ui.rs (not staged):
- Added Paragraph to ratatui::widgets import
- Added render_session_info() function (Info block: title/agent/model/parent/tokens/errors)
- Added render_detail_pane(frame, app, area, is_focused) replacing render_events_pane
- Removed old render_events_pane (its logic is now in render_detail_pane)
- Updated render_activity callers: render_detail_pane(... false) for Split, render_detail_pane(... true) for DetailFull

- **Found:** Steps 1-3 all compiled and tested clean when tested individually (83/90 tests green at each step)
- **Found:** Step 4 code is syntactically sound — LSP shows no errors after edits — but cargo build/check fails with rustc EAGAIN panic on ctrlc handler install, which is a transient OS resource limit, not a code error
- **Found:** The workspace test run failure (27 failures in 'test result: FAILED') are from the acuity integration test suite and appear pre-existing — they fail regardless of curator changes
- **Found:** The acuity integration tests use tokio and network binds which may exhaust the same OS resources
- **Decided:** Did not commit step 4 — pre-commit validation requires green tests, and tests cannot run while rustc panics. Left changes unstaged for the next agent to verify and commit once the system recovers
- **Decided:** The three committed steps (1-3) are correct and independently verified
- **Open:** Verify step 4 compiles and tests pass once system resources recover (retry cargo test -p curator)
- **Open:** Commit step 4 with message: feat(curator): detail pane with session Info block + gitui layout
- **Open:** Run cargo test --workspace and cargo clippy --workspace -- -D warnings to confirm full workspace green
- **Open:** Update plan status to in-progress in slice6d-gitui-ux.md
- **Open:** Step 5: manual live QA (14 criteria in the plan)

## [633522a] [633522a] Slice 6d step 4 shipped — detail pane + gitui layout complete

Commit 633522a on feat/curator-activity-item. Step 4 of slice6d-gitui-ux.md was code-complete but unverified at last session end (blocked by transient OS EAGAIN). System resources recovered; cargo test -p curator ran clean (90/90), clippy clean, full workspace all green (no failed suites).\n\nChanges in ui.rs:\n- Added render_session_info() — Info block showing title, agent, model, parent_id, input/output tokens, error_count from SessionSummary\n- Added render_detail_pane(frame, app, area, is_focused) — top Info block + bottom Events list; is_focused controls highlight and border style\n- Removed old render_events_pane (its logic folded into render_detail_pane)\n- render_activity updated: Split calls render_detail_pane(..., false), DetailFull calls render_detail_pane(..., true)\n\nAll 4 slice6d commits now shipped: helpers (84908d6), columnar rows (7ef8690), ActivityLayout refactor (27dceba), detail pane (633522a).\n\nPlan status: in-progress (step 5 manual live QA pending).

- **Found:** 90/90 curator tests passed after system recovery — step 4 code was correct, only blocked by OS resource exhaustion
- **Found:** Full workspace: all test suites green, clippy clean across all crates
- **Open:** Step 5: manual live QA — 14 criteria in slice6d-gitui-ux.md (three-state layout, columnar rows, Enter/Right/Esc navigation, Info block content, Diagnostics unaffected, etc.)

## [c798df4] [c798df4] Sessions pane layout fix + info pane improvements + local TZ events

Commit c798df4 on feat/curator-activity-item. Addresses all user feedback from Slice 6d manual QA on the left and right panes.

- **Found:** trunc_pad helper was required to fix column alignment: format!("{:<8}") pads to minimum width but does not truncate, so project names longer than 8 chars (e.g. nix-config) broke column alignment
- **Found:** is_multiple_of(3) is stable in current Rust toolchain — clippy suggested it over the manual % 3 == 0 expression
- **Found:** Nested if-let chains required collapsing all the way including the bool guard (seen.insert(...)) into the && chain to satisfy clippy
- **Decided:** Sessions pane: project col 20 chars (trunc_pad), datetime col 10 chars (HH:MM:SS today / YYYY-MM-DD other), harness 2 chars unchanged
- **Decided:** Session Info block: renamed to Session Info, added ID + Project rows, Agents/Models show unique lists scanned from ring buffer events with SessionSummary fallback for agents, tokens use comma separators, height 10
- **Decided:** Events pane: timestamps converted to local TZ via format_event_datetime, color changed from DarkGray to Yellow to avoid collision with highlight background

## [54ddc6c] [54ddc6c] Session list polish — column order, colors, tokens split, no arrow symbol

- **Found:** LightCyan is the correct ratatui color for bright cyan; using it for both sessions datetime and events datetime gives a consistent visual language
- **Found:** Removing highlight_symbol reclaims 2 chars per row; the DarkGray background highlight is sufficient for selection indication
- **Decided:** Column order: harness (2, fixed) | datetime (10) | project (20) | title (fill) — most-predictable columns on left
- **Decided:** Datetime color: LightCyan in both sessions pane and events pane (was Cyan/Yellow respectively)
- **Decided:** Tokens split into Tokens In / Tokens Out rows; all labels right-aligned to 13 chars for consistent column; info block height 10 -> 11
- **Decided:** highlight_symbol removed from Sessions list and Events list (activity view only; kanban and diagnostics unchanged)
- **Decided:** Clipboard keybinding (C-y) deferred as todo — requires new crate dependency (arboard)
- **Open:** C-y copy session_id to clipboard — captured as todo, needs arboard or xclip shell-out dependency decision

## [4a2d9bb] Session title color changed to white in sessions list

Commit 4a2d9bb on feat/curator-activity-item. Changed the session title color in the Sessions pane from Cyan to White (ui.rs:211). Placeholder case (no title) remains DarkGray. 99 curator tests pass, clippy clean.

- **Decided:** White+Bold for titled session rows; DarkGray retained for placeholder rows

## [4a2d9bb] Branch todo housekeeping — timezone resolved, 4 todos elevated to master tasks

Analyzed all 7 todo artifacts on feat/curator-activity-item ahead of branch wrap-up. None warranted blocking work on this branch.

Actions taken (all in .cue/, no code changes, no git commits):
1. fix-displayed-timezone-of-received-at: marked complete. Was already resolved in code by commits c798df4 + 54ddc6c (both datetime formatters use with_timezone(&Local)) — the todo was stale.
2. Four open low-priority todos elevated to formal master tasks, then source todos closed:
   - normalize-events-sessions-table -> master/task/db-sessions-table-normalization.md
   - collect-user-turn-messages -> master/task/collect-user-turn-messages.md
   - improve-session-data-loading -> master/task/improve-session-data-loading.md
   - clipboard-copy-session-id -> master/task/clipboard-copy-session-id.md
3. Two QA-checklist todos (slice6a-manual-qa, slice6b-manual-qa) left as-is — already status:complete, they are historical records of the live verification performed.

- **Decided:** Timezone todo was already resolved in code (c798df4/54ddc6c) — marked complete rather than left dangling
- **Decided:** Elevated 4 open todos to formal master tasks with acceptance criteria, then closed the source todos per cue protocol
- **Decided:** Kept the 2 QA-checklist todos untouched — they are already complete and serve as historical QA records
- **Decided:** No code changes needed — none of the open todos warranted work on this branch

## [c6d3932] Commit c6d3932 — clamp sel_activity on eviction + info block doc fix

Commit c6d3932 on feat/curator-activity-item. Step 1 of slice6e-review-fixes.md (TDD).

Added a clamp in ensure_session_selection's else-branch: when the selected session is still valid (visible events > 0) but sel_activity exceeds the current visible count - 1, it is clamped. This fixes the stale-index bug where partial ring-buffer eviction stranded the highlight.

Also fixed the render_session_info doc comment (10 -> 11 rows, 8 -> 9 data lines) to match the actual Constraint::Length(11) and 9 data lines.

Updated ensure_session_selection_is_idempotent: the old test set sel_activity=5 for a 1-event session (itself stale). Now uses 10 events with sel_activity=3 (valid). Added ensure_session_selection_clamps_stale_sel_activity as the new RED->GREEN test for the clamp behavior.

100/100 curator tests pass, clippy clean.

- **Decided:** Clamp lives in the else-branch of ensure_session_selection (only when session is valid) — the needs_reset path already sets sel_activity=0
- **Decided:** Updated the idempotency test to use a valid sel_activity (3 within 10 events) instead of the unrealistic 5 within 1 event — the old value was itself stale and would have been clamped by the fix

## [61ee148] Commit 61ee148 — cache per-session aggregates on SessionSummary (perf fix #2 + O-5/O-6)

Commit 61ee148 on feat/curator-activity-item. Step 2 of slice6e-review-fixes.md.

Added three cached fields to SessionSummary: visible_event_count (u32), unique_agents (Vec<String>), unique_models (Vec<String>). All maintained incrementally in push_event:

- visible_event_count: incremented for non-hidden events on append, decremented (saturating) for non-hidden events on ring-buffer eviction. session_event_len now reads this field directly (O(1) instead of O(N) ring scan). sorted_sessions goes from O(N*CAP) to O(N).

- unique_agents/unique_models: accumulated inside the existing session_updated and agent_turn_completed match arms (no extra deserialization). Deduped via Vec::contains (small n). session_unique_agents/session_unique_models in ui.rs are now thin readers returning s.unique_agents.clone() / s.unique_models.clone() — no more per-frame serde_json parsing.

The cached lists survive ring-buffer eviction (they live on SessionSummary), which is better than the old behavior (scan the ring buffer, fall back to last-writer-wins when evicted).

Also fixes O-5 (asymmetric fallback): both agents and models now source symmetrically from the cached Vec.

Updated all 3 SessionSummary construction sites with the new fields. push_turn_at test helper now increments visible_event_count manually (it bypasses push_event).

105 curator tests pass (100 existing + 5 new: visible_event_count_tracks_eviction, unique_agents_accumulate_and_survive_eviction, unique_models_accumulate_from_turn_and_session_updated, unique_agents_dedup_repeats, unique_models_dedup_same_model_across_turns). Clippy clean (collapsed eviction let-chain per clippy::collapsible_if).

- **Found:** push_turn_at test helper bypasses push_event (pushes directly to app.events and manually manages app.sessions) — needed manual visible_event_count increment added
- **Found:** The existing session_event_len tests (session_event_len_counts_only_target_session, session_event_len_excludes_hidden_events) are implementation-agnostic and validated the cache maintenance without modification
- **Found:** Clippy::collapsible_if flags nested if-let patterns that can be collapsed to let-chains — the codebase already uses let-chains elsewhere (app.rs:288 tool_call_completed arm)
- **Decided:** unique_agents/unique_models use Vec<String> with .contains() dedup (not HashSet) — n is tiny (1-5 per session), preserves first-seen order, simpler API
- **Decided:** Cached lists live on SessionSummary (survives eviction by design) not on App — consistent with the existing 'totals survive eviction' contract on SessionSummary
- **Decided:** Maintained inside existing match arms to avoid double-deserialization of the payload
- **Decided:** visible_event_count uses saturating_sub on eviction to guard against any drift
- **Decided:** Eviction let-chain collapsed per clippy::collapsible_if (4 nested ifs -> one if-let-chain, matching the existing pattern at app.rs:288)

## [1839275] Commit 1839275 — hoist frame clock + inline active styles

Commit 1839275 on feat/curator-activity-item. Step 3 of slice6e-review-fixes.md.

Two polish fixes:
- O-4: Split format_datetime into format_datetime_on(ts, today) that takes a precomputed NaiveDate. render_sessions_pane computes today once per frame. Removed the format_datetime wrapper (it became dead code since production now calls _on directly; tests updated to call _on with Local::now().date_naive()).
- O-3: Removed the dead `let is_active = true;` and inlined the active highlight_style and border_style directly in render_sessions_pane. The DIM/DarkGray else branches were permanently dead.

105 curator tests pass, clippy clean.

- **Decided:** Removed format_datetime wrapper entirely rather than marking #[allow(dead_code)] — the wrapper was genuinely unused in production, and adding an allow is itself a code smell
- **Decided:** Tests call format_datetime_on directly with Local::now().date_naive() — today is irrelevant for parse-failure tests and for the past-date test any modern today works

## [92605bc] Slice 6e review fixes complete — 4 commits, Opus SHIP verdict, 106 curator tests green

All four steps of slice6e-review-fixes.md are complete. Four commits on feat/curator-activity-item:

1. c6d3932 fix(curator): clamp sel_activity on ring-buffer eviction
   - Fixes #1 (Major correctness): stale sel_activity after partial ring-buffer eviction
   - Also fixes O-2 (Info block doc: 10 -> 11 rows)

2. 61ee148 perf(curator): cache per-session aggregates on SessionSummary
   - Fixes #2 (Major perf): eliminates per-frame ring-buffer scan + serde_json::from_str in session_unique_agents/session_unique_models
   - Fixes O-5 (asymmetric fallback): both agents and models now source symmetrically from cached Vec
   - Fixes O-6 (Minor perf): session_event_len is O(1) instead of O(N), sorted_sessions is O(N) instead of O(N*CAP)
   - Added visible_event_count, unique_agents, unique_models to SessionSummary, maintained incrementally in push_event

3. 1839275 style(curator): hoist frame clock, inline active session styles
   - Fixes O-4 (Local::now() per row -> once per frame via format_datetime_on)
   - Fixes O-3 (dead is_active=true branch removed, active styles inlined)

4. 92605bc test(curator): verify visible_event_count recovers after full eviction
   - Belt-and-suspenders edge-case test from Opus review feedback

Consultant-opus review verdict: SHIP. All three fix commits correct, cache invariant proven airtight, no new issues. Two optional follow-ups were suggested (the edge-case test and defensive-guard comments); the test was added in commit 4.

Full workspace: all test suites green (106 curator tests), clippy clean across all crates. Plan status: complete.

- **Found:** The ensure_session_selection_is_idempotent test was itself testing broken behavior (sel_activity=5 for a 1-event session was an out-of-range index that should have been clamped) — updated to use a valid index
- **Found:** push_turn_at test helper bypasses push_event (pushes directly to app.events) and needed manual visible_event_count increment to stay compatible with the cached session_event_len
- **Found:** format_datetime wrapper became dead code after splitting to format_datetime_on — removed the wrapper rather than marking #[allow(dead_code)]
- **Decided:** Skipped O-7 (redundant harness re-clone in session_updated arm) — defense in depth, one clone of a short string on deduped events, not worth the risk of removing
- **Decided:** Skipped F-1 (hardcoded UTF-8 char constants) — purely stylistic, no behavioral impact
- **Decided:** Skipped F-2 (session_label title clone per frame) — n < 20 sessions, negligible allocation; Cow return type adds API complexity not worth the micro-optimization
- **Decided:** Skipped O-1 (Split->DetailFull no sel_activity reset) — transitively resolved by the clamp fix in commit 1; entering DetailFull always has a valid clamped index
- **Decided:** Left activity.rs / activity_len dead code as-is — tracked debt for stage C, well-documented

