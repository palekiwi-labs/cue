---
status: complete
---
## Foreword

Implements the **collection layer** for stage-B lineage display in the curator
activity feed. Adds one new acuity event variant `SessionUpdated`, emitted by
the plugin on opencode's `session.created` + `session.updated`, carrying
`parent_id`, `agent`, `model`, and `title`. Curator ingests it into
`SessionSummary`.

**No DB column changes. No changes to the existing 4 event variants.** The
metadata lives only inside `SessionUpdated` payloads and curator's in-memory
`SessionSummary` (which already survives ring-buffer eviction by design —
`app.rs:73-74, 236-242`).

This follows Option L (lean) from the Opus consultation. The decision NOT to
denormalize `parent_id`/`agent`/`model` onto every events row (Option V) is
recorded in the todo `normalize-events-sessions-table.md`: the display path is
already a render-time join into `sessions`; per-row columns would be redundant
with `SessionSummary` and insufficient for stage-C tree construction (the real
parent→child edge is the Task-tool-completed result already in `payload`).

**Branch:** `feat/curator-activity-item`
**Test exit:** `cargo test` (workspace) green + `cargo clippy --workspace -- -D warnings` clean.
**Exit criterion (live):** with the stack running, `SELECT session_id, parent_id, agent, model, title FROM events WHERE event_type='session_updated'` shows correct lineage, and the title row precedes the `session_idle` row (early title).

**Context that motivated this slice** (from DB ground truth + opencode source):
- Sub-agent sessions are distinct sessions (`task.ts:129-145` sets `parentID`),
  running concurrently and interleaving by arrival time. The plugin captured no
  lineage, agent, or model.
- The title only arrived at idle because `session.idle` was the sole handler
  reading it (`acuity-plugin.ts:71-87`). opencode regenerates the title seconds
  after the first prompt via `ensureTitle` (`prompt.ts:226-285`) → `setTitle` →
  `patch`, which publishes `session.updated` with full info (`session.ts:776-788`).
- opencode's `Session.Info` schema has `parentID`, `agent`, `model`, `title`
  (`session/schema.ts:27-46`); `session.created`/`session.updated` events carry
  `properties.info: Session` (`types.gen.ts:533-547, 562-574`).

### Design decisions (locked)

- **Option L (lean).** One new event variant + curator join. No new columns.
- **`model` as flat string** `"providerID/modelID"`.
- **Existing 4 variants untouched.** `SessionUpdated` owns all session metadata.
- **`SessionUpdated` carries the `project_dir`/`harness` envelope** so the
  `project_dir()`/`harness()` enum accessors stay exhaustive and `insert_event`'s
  NOT NULL column binds remain valid. This is a new variant, so it does not
  violate "leave existing variants untouched".
- **No `SessionUpdated` on idle** (keep `session_idle` → `SessionIdle` only).
  Fallback if 6a testing reveals lineage gaps for sessions that never fire
  `session.updated`: also emit `SessionUpdated` from the idle handler.
- **`parent_id` is set-once-if-some** in `SessionSummary` (guard against a later
  `null` clobbering a known parent), mirroring the `project_dir` guard at
  `app.rs:203`. `title`/`agent`/`model`/`harness` are last-writer-wins (only
  when `Some`).

### Steps

- [x] **1. `crates/acuity-schema/src/lib.rs` — add `SessionUpdated` variant.**
  New struct `SessionUpdated { session_id, project_dir, harness, parent_id: Option<String>, agent: Option<String>, model: Option<String>, title: Option<String> }` with the same derives/attrs as the others.
  Add `SessionUpdated(SessionUpdated)` to the `AcuityEvent` enum (`lib.rs:90-95`).
  Extend `event_type()` (`lib.rs:99-106`) → `"session_updated"`; `session_id()` (`lib.rs:109-116`) → `&e.session_id`; add a `None` arm for it in `turn_id()` (`lib.rs:139-146`); add uniform `&e.project_dir` / `&e.harness` arms in those accessors (`lib.rs:118-136`).
  Tests: round-trip, `event_type` discriminant, raw-JSON deserialize; confirm `unknown_fields_are_ignored_on_deserialization` still passes.

- [x] **2. Regenerate TypeScript types.**
  Run the `ts_rs` codegen (`crates/acuity-schema/src/bin/codegen.rs`). Confirm `generated/acuity/types.ts` exports `SessionUpdated`, which the plugin imports at `acuity-plugin.ts:3-9`.

- [x] **3. Plugin (`acuity-plugin.ts`) — add `session.created` + `session.updated` handlers.**
  For both: `info = event.properties.info`; extract `session_id = info.id`, `parent_id = info.parentID ?? null`, `agent = info.agent ?? null`, `model = info.model ? \`${info.model.providerID}/${info.model.modelID}\` : null`, `title = info.title ?? null`, `project_dir = directory`, `harness = "opencode"`; `postEvent`.
  **Type caveat:** the V1 SDK `Session` type (`types.gen.ts:533-547`) declares `parentID` but may omit `agent`/`model` at the type level even though the runtime object carries them — access defensively (`(info as any).agent`) with a comment.
  Leave the existing `session.idle` handler unchanged.

- [x] **4. Curator `crates/curator/src/app.rs` — extend `SessionSummary` + `push_event`.**
  `SessionSummary` (`app.rs:76-91`) gains `parent_id: Option<String>`, `agent: Option<String>`, `model: Option<String>`, `harness: String`; initialise in `or_insert_with` (`app.rs:183-192`).
  Add a `"session_updated"` match arm in `push_event` (`app.rs:207`): deserialize `AcuityEvent::SessionUpdated` from `payload` (mirroring how `session_idle` deserializes for the title at `app.rs:209-213`); last-writer-wins for `title`/`agent`/`model`/`harness` (only when `Some`); set-once-if-some for `parent_id`.

- [x] **5. Curator tests (`app.rs`).**
  Add: `push_session_updated_populates_summary`, `parent_id_set_once_not_clobbered_by_later_null`, `title_updates_last_writer_wins`, `agent_and_model_populated`. Add a `session_updated(...)` test helper.

- [ ] **6. Verify + commit.**
  Automated verify DONE: `cargo test --workspace` green (schema 25, curator 47, all crates pass) + `cargo clippy --workspace -- -D warnings` clean. Commits shipped (schema variant 7f4bac5, curator ingest 1acf52f, plugin handlers 8555572 in cue-plugins).
  **Manual E2E (deferred to user):** rebuild plugin, restart acuity with a **dropped/recreated** DB, run an opencode session that spawns a sub-agent; confirm lineage rows are correct and the title row precedes the `session_idle` row.

### Out of scope

- 6b: rendering changes, killing the 8-char truncation, debug columns.
- Stage C: nested tree, folding child sessions under parent turns.
- Sessions-table normalization (see todo `normalize-events-sessions-table.md`).
