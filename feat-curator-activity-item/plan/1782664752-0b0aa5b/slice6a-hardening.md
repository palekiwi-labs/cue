---
status: complete
---
## Foreword

This plan hardens Slice 6a (the `SessionUpdated` collection layer) based on
findings from **live QA** and a **deep opencode source investigation**. It is
fully self-contained — a fresh agent can execute it from this document alone.

### What's already shipped (Slice 6a, commits on `feat/curator-activity-item`)

- `7f4bac5` — `SessionUpdated` schema variant (`acuity-schema/src/lib.rs`)
- `1acf52f` — Curator ingest into `SessionSummary` (`curator/src/app.rs`)
- `8555572` (cue-plugins repo) — Plugin `session.created`/`session.updated`
  handlers + regenerated `types.ts`

The pipeline works end-to-end (plugin -> acuity server -> SQLite -> curator).
Live QA confirmed: lineage (`parent_id`) captures correctly, agent captures
correctly, title arrives early (before `session_idle`).

### What the live QA revealed (two problems)

**Problem 1 — Severe `session_updated` duplication.** opencode fires
`session.created` + multiple `session.updated` for the same state. The handler
emits a `SessionUpdated` for each with no dedup. Fresh DB evidence: **5
identical `session_updated` rows** for a single session (seq 1,2,6,9,14 — all
same agent/title/model). In an earlier dump with a sub-agent: **13 of 22
events** (59%) were `session_updated`, only ~3 carrying distinct information.

**Problem 2 — `model` is always `null`.** The opencode `Session` runtime object
carries `agent` but NOT `model` for sessions created without an explicit model
(notably Task-tool subagent sessions — `task.ts:129-145` calls
`sessions.create({ parentID, title, agent, permission })` with no model). The
model is resolved **per-turn at runtime** via a fallback chain and is only
visible in turn-level events. See the full investigation at
`.cue/feat-curator-activity-item/doc/1782664752-0b0aa5b/opencode-model-capture.md`.

### Version situation

- opencode runtime: **`1.17.11`** (user updated)
- installed SDK in cue-plugins node_modules: **`1.17.9`** (stale — `package.json`
  spec is `"*"` which resolved to 1.17.9)
- The ref/opencode source (matching latest) types `agent?: string` and
  `model?: { id, providerID, variant? }` on `Session`, and exposes
  `session.next.step.started`. Pinning the SDK to 1.17.11 should bring these
  types (verify after install).
- `AssistantMessage` in SDK 1.17.9 already has `modelID: string` and
  `providerID: string` as **required typed fields** (`types.gen.d.ts:108-109`).

### Repos and paths

- **cue repo:** `/home/pl/code/palekiwi-labs/cue` (branch `feat/curator-activity-item`)
- **cue-plugins repo:** `/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins` (branch `master`)
- **Running acuity DB:** `.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity/events.db` (server started by user with `ACUITY_PORT=33222 ACUITY_DATA_DIR=.cue/feat-curator-activity-item/tmp/acuity-phase-6a`)
- **cue-plugins typecheck/build:** must use `nix develop -c bash -c "bun run typecheck"` from the cue-plugins directory (bun is only in the nix devshell)

### Design decisions (locked)

- **Model capture goes on `AgentTurnCompleted`**, not `SessionUpdated`. The
  model is resolved per-turn; `AgentTurnCompleted` IS the turn-level event and
  fires from `message.updated` (assistant, completed). The assistant message
  carries typed `modelID`/`providerID`. This captures the resolved model for ALL
  sessions including subagents, which `SessionUpdated` cannot do.
- **`SessionUpdated.model` is kept** (not removed). It cooperates with the new
  `AgentTurnCompleted.model` via the curator's last-writer-wins. It's sometimes
  useful (explicit-model sessions) and costs nothing.
- **Dedup uses a persistent `lastSessionSig` Map** (not the existing
  turn/call `dedup` Map, which is cleared on idle). This collapses identical
  consecutive session events across idle boundaries.
- **No `session.next.step.started` handler.** Available in 1.17.11 types but
  unnecessary — `message.updated` already provides the model at turn completion,
  which is when the activity feed displays it.

### Steps

- [ ] **1. Pin SDK version (cue-plugins repo, commit 1)**

  Edit `package.json` (`/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins/package.json`):
  change both `"*"` specs to `"1.17.11"`:
  ```json
  "@opencode-ai/plugin": "1.17.11",
  "@opencode-ai/sdk": "1.17.11",
  ```
  Then install and typecheck:
  ```bash
  cd /home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins
  nix develop -c bash -c "bun install && bun run typecheck"
  ```
  **Verify:** after install, confirm the `Session` type in
  `node_modules/@opencode-ai/sdk/dist/gen/types.gen.d.ts` now includes `agent?`
  and `model?`. If it does, proceed to cast cleanup (step 2). If not (SDK 1.17.11
  still lags), keep the existing casts and note it in the commit body.
  Commit: `chore: pin @opencode-ai/sdk and plugin to 1.17.11`

- [ ] **2. Cast cleanup in SessionUpdated handler (cue-plugins repo, fold into commit 1)**

  In `src/opencode/acuity-plugin.ts`, the session.created/session.updated
  handler (around line 80-100) uses defensive casts. After pinning, IF the SDK
  Session type now declares `agent?`/`model?`, simplify:
  - `(info as { agent?: string }).agent` -> `info.agent`
  - The `(info as { model?: unknown }).model as { providerID?: ... }` chain ->
    typed `info.model` access: `info.model ? \`${info.model.providerID}/${info.model.id}\` : null`
    (note: the typed shape is `{ id, providerID, variant? }`, NOT `modelID` —
    use `model.id`)
  - `info.parentID` and `info.title` are already typed (no change needed).
  If the SDK type still lacks these fields, leave the casts as-is.
  Fold into commit 1.

- [ ] **3. Dedup session_updated events (cue-plugins repo, commit 2)**

  In `src/opencode/acuity-plugin.ts`:
  - Add a module-level persistent map (near the existing `dedup` declaration,
    around line 16): `const lastSessionSig = new Map<string, string>();`
  - In the session.created/session.updated handler, AFTER computing the payload
    fields but BEFORE `postEvent`, add:
    ```typescript
    const sig = JSON.stringify([
      payload.parent_id, payload.agent, payload.model, payload.title,
    ]);
    if (lastSessionSig.get(info.id) === sig) return;
    lastSessionSig.set(info.id, sig);
    ```
  This skips identical consecutive session events. It persists across idle
  (unlike the turn/call `dedup` which is cleared on `dedup.delete(sessionID)`).
  Commit: `fix(acuity-plugin): dedup session_updated events by metadata signature`

- [x] **4. Add model field to AgentTurnCompleted (cue repo, commit 3, TDD)**

  Commit `abf7772`. Schema field added and tested. 25/25 green, clippy clean.
  - RED/GREEN cycle completed for `crates/acuity-schema/src/lib.rs`.
  - **Outstanding:** types.ts not yet regenerated; `model = ?e.model` not yet
    added to the `AgentTurnCompleted` log arm in `main.rs`. Both are handled
    as part of B3 follow-up steps in `slice6a-logging-and-plugin.md`.

- [x] **5. Capture model in plugin message.updated handler (cue-plugins repo, commit 4)**

  Commit `3aeb835`.

- [x] **6. Ingest model in curator (cue repo, commit 5, TDD)**

  Commit `d590d61`. RED/GREEN cycle completed. 48/48 curator tests pass.
  Updated test helpers in activity.rs (x2) and ui.rs (x1) with `model: None`.
  `turn()` helper in app.rs uses `model: Some("anthropic/claude-sonnet")`.

- [x] **7. Final verification**

  - `cargo test --workspace` — 304 tests, all green
  - `cargo clippy --workspace -- -D warnings` — clean
  - `nix develop -c bash -c "bun run typecheck"` (cue-plugins) — clean
  - Live QA confirmed via `tmp/acuity.log`: session_updated dedup collapsed
    from 5+ to 2 per session; agent_turn_completed carries non-null model
    on every turn including subagents (google/gemini-3-flash-preview).

### Out of scope

- `session.next.step.started` handler (future option for turn-start model capture)
- Removing `model` from `SessionUpdated` (kept — cooperates via last-writer-wins)
- Slice 6b rendering (separate plan)
- Sessions-table normalization (existing deferred todo)

### Key references for the executing agent

- Investigation doc: `.cue/feat-curator-activity-item/doc/1782664752-0b0aa5b/opencode-model-capture.md`
- DB trace (live QA evidence): `.cue/feat-curator-activity-item/trace/1782664752-0b0aa5b/db.json`
- Manual QA checklist: `.cue/feat-curator-activity-item/todo/1782664752-0b0aa5b/slice6a-manual-qa.md`
- Original 6a plan: `.cue/feat-curator-activity-item/plan/1782659497-0c2ff37/slice6a-collection.md`
