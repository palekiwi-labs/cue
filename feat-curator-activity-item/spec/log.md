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

