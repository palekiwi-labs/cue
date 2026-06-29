---
status: complete
---
## Foreword

Hardens the Slice 6a `SessionUpdated` collection layer by:
1. Adding rich observability to the `acuity` server so plugin behavior
   is verifiable in real-time via logs (Phase A).
2. Fixing the two live-QA defects in the plugin — session_updated duplication
   and model-always-null — verified via those logs (Phase B).

**Curator changes are deferred.** All `curator` work (original step 6:
ingest `model` from `AgentTurnCompleted`) waits until Phase B is live-verified.

### What is already shipped (Slice 6a base)

- `7f4bac5` — `SessionUpdated` schema variant (`acuity-schema/src/lib.rs`)
- `1acf52f` — Curator ingest into `SessionSummary` (`curator/src/app.rs`)
- `8555572` (cue-plugins) — Plugin `session.created`/`session.updated` handlers

### Two live-QA defects being fixed here

**Defect 1 — session_updated duplication.** opencode fires
`session.created` + multiple `session.updated` for the same state.
The plugin handler (`acuity-plugin.ts:76-100`) emits a `SessionUpdated`
for each with no dedup. Fresh DB: 5 identical rows per session.

**Defect 2 — model is always null.** `info.model` is `undefined` for
sessions created without an explicit model (Task-tool subagent sessions,
`task.ts:129-145`). The model is resolved per-turn at runtime and is only
visible in turn-level events (`message.updated` assistant message carries
typed `modelID`/`providerID`).

### Repos and paths

- **cue repo:** `/home/pl/code/palekiwi-labs/cue`
  branch `feat/curator-activity-item`
- **cue-plugins repo:** `/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins`
  branch `master`
- **Dev DB + log dir:** `.cue/feat-curator-activity-item/tmp/acuity-phase-6a/`
- **Log file (created by Phase A):**
  `.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log`
- **Typecheck/build:** `nix develop -c bash -c "bun run typecheck"` from cue-plugins

### Design decisions (locked)

- **Dual-filter subscriber** — stderr (terminal) at `RUST_LOG`/`acuity=info`;
  file layer at `ACUITY_LOG_LEVEL`/`acuity=debug`. Terminal stays quiet for
  the human; file is rich for agent realtime analysis.
- **Truncate-on-start for the log file** — each `cargo run` session is a
  fresh observable experiment. The user can preserve history by renaming
  the file before restart if needed.
- **`ACUITY_LOG_FILE` env var** — no default path; must be set explicitly.
  Suggested path: `.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log`
  (inside the workspace mount, so agent can read it via Read/bash tools).
- **No new dependencies** — `Mutex<W>: MakeWriter` is built into
  `tracing-subscriber` 0.3; existing `features = ["env-filter"]` plus
  defaults covers `registry` + `fmt`.
- **Per-variant real tracing fields** (Opus recommendation) — not a single
  opaque `summary` string. `Option<String>` uses `?` (Debug), not `%`
  (Display) — `Option<T>` does not implement Display.
- **DEBUG line: both `%payload` + `?event`** — raw-vs-parsed delta is the
  30-second triage for plugin-vs-schema deserialization bugs.
- **Model on `AgentTurnCompleted`** (not SessionUpdated) — model is
  resolved per-turn; `message.updated` assistant message carries typed
  `modelID`/`providerID` (required fields, no cast needed).
- **`lastSessionSig` Map is persistent** (not cleared on idle, unlike the
  turn/call `dedup` Map) — deduplicates identical consecutive session events
  across session-idle boundaries.

---

## Phase A — Acuity Observability

**Crate:** `crates/acuity`  
**Commit:** `feat(acuity): structured per-variant logging + optional file output`  
**Test exit:** `cargo test -p acuity` green + `cargo clippy -p acuity -- -D warnings` clean.

- [x] **A1. Add `#[derive(Debug)]` to `AcuityEvent` and all inner structs**
  (`crates/acuity-schema/src/lib.rs`) if absent. Required for `?event` in
  the DEBUG line. Verify with a `cargo check -p acuity-schema`.
  *No-op: `Debug` was already derived on all structs and the enum.*

- [x] **A2. Refactor subscriber to multi-layer in `crates/acuity/src/main.rs:200-205`.**

  Replace:
  ```rust
  tracing_subscriber::fmt()
      .with_env_filter(
          tracing_subscriber::EnvFilter::try_from_default_env()
              .unwrap_or_else(|_| "acuity=info".into()),
      )
      .init();
  ```

  With a `Registry` + two layers (stderr + optional file):
  ```rust
  use tracing_subscriber::{fmt, prelude::*, EnvFilter};

  let stderr_filter = EnvFilter::try_from_default_env()
      .unwrap_or_else(|_| EnvFilter::new("acuity=info"));
  let stderr_layer = fmt::layer()
      .with_writer(std::io::stderr)
      .with_filter(stderr_filter);

  let file_layer = std::env::var("ACUITY_LOG_FILE").ok().map(|path| {
      let f = std::fs::OpenOptions::new()
          .create(true)
          .write(true)
          .truncate(true)      // truncate-on-start: clean log per dev session
          .open(&path)
          .unwrap_or_else(|e| panic!("ACUITY_LOG_FILE ({path}): {e}"));
      let writer = std::sync::Mutex::new(std::io::BufWriter::new(f));
      let file_filter = EnvFilter::try_from_env("ACUITY_LOG_LEVEL")
          .unwrap_or_else(|_| EnvFilter::new("acuity=debug"));
      fmt::layer()
          .with_writer(writer)
          .with_ansi(false)
          .with_filter(file_filter)
  });

  tracing_subscriber::registry()
      .with(stderr_layer)
      .with(file_layer)
      .init();
  ```

  Required import additions at top of file (alongside existing `use` statements).

- [x] **A3. Replace single `info!` at `main.rs:340-345` with per-variant match.**

  The existing call:
  ```rust
  info!(
      seq,
      event_type = event.event_type(),
      session_id = event.session_id(),
      "persisted event"
  );
  ```

  Replace with a match on `&event`. Common fields (`seq`, `event_type`,
  `session_id`) are repeated per arm; do NOT duplicate within the same arm.
  Use `?` for `Option<String>` fields, `%` for `String` fields.

  Arms implemented (commit 369a0d1):
  - `SessionUpdated(e)`: `parent_id = ?e.parent_id`, `agent = ?e.agent`,
    `model = ?e.model`, `title = ?e.title`
  - `AgentTurnCompleted(e)`: `turn_id = %e.turn_id`,
    `input_tokens = ?e.input_tokens`, `output_tokens = ?e.output_tokens`
    (`model` omitted here — add `model = ?e.model` as follow-up in B3)
  - `SessionIdle(e)`: `title = ?e.session_title`
  - `ToolCallRequested(e)`: `turn_id = %e.turn_id`, `tool = %e.tool_name`
  - `ToolCallCompleted(e)`: `turn_id = %e.turn_id`, `tool = %e.tool_name`,
    `is_error = e.is_error`

- [x] **A4. Add DEBUG line after the INFO block (inside the `Ok(seq)` arm).**

  ```rust
  tracing::debug!(seq, payload = %payload, event = ?event, "raw payload + parsed event");
  ```

  This gives the raw-vs-parsed delta: if `model` is absent in `payload`
  (plugin sent no field) vs absent in `?event` (schema dropped it on
  deserialization) — different failure modes, different fixes.

  Note: `payload` (and `ToolCallRequested.args`) may contain secrets
  (e.g. bash tool args with env vars). This is gated at DEBUG only.
  The DB already stores `payload` verbatim (same exposure class).
  Redaction is out of scope for the local-dev threat model.

- [x] **A5. Log the file path at startup** (after the subscriber is init'd):
  ```rust
  if let Ok(path) = std::env::var("ACUITY_LOG_FILE") {
      info!("file logging enabled: {path}");
  }
  ```

---

## Phase B — Plugin + Schema Fixes

Verify each step by running an opencode session and reading the log file
at `.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log`.

### B1+B2: Plugin SDK pin + dedup (cue-plugins repo)

- [x] **B1. Pin SDK to 1.17.11** (`package.json`).
  Commit `f3b11f7`. SDK 1.17.11 `Session` type still omits `agent?`/`model?`
  — defensive casts stay.

- [x] **B1b. Cast cleanup (folded into B1 commit `f3b11f7`).**
  SDK 1.17.11 still lacks `agent?`/`model?` on `Session`, so casts stay.
  Fixed the model cast field name: `modelID` → `id` (Session.model runtime
  shape is `{ id, providerID }`, not `{ modelID, providerID }`).

- [x] **B2. Add persistent `lastSessionSig` Map for dedup.**
  Commit `db90d51`.

### B3: Schema model field (cue repo, TDD)

- [x] **B3. Add `model: Option<String>` to `AgentTurnCompleted`**
  (`crates/acuity-schema/src/lib.rs:30-43`). Commit `abf7772`.
  - RED: updated `agent_turn_completed()` test helper and
    `agent_turn_completed_deserializes_from_raw_json` to include `model`.
    Confirmed compile error.
  - GREEN: added `pub model: Option<String>` to struct. 25/25 tests pass,
    clippy clean.
  - **Complete (commit `f5c1695` in cue repo + `3aeb835` in cue-plugins):**
    - [x] Regenerate types.ts:
      `cargo run -p acuity-schema --bin codegen -- /home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins/src/generated/acuity`
    - [x] Add `model = ?e.model` to the `AgentTurnCompleted` arm in
      `crates/acuity/src/main.rs` (commit `f5c1695`).

### B4: Plugin model capture (cue-plugins repo)

- [x] **B4. Capture resolved model in `message.updated` handler.**
  Commit `3aeb835`.

---

## Verification Checklist (real-time via log file)

Run acuity with:
```bash
ACUITY_LOG_FILE=/home/pl/code/palekiwi-labs/cue/.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log \
ACUITY_PORT=33222 \
ACUITY_DATA_DIR=.cue/feat-curator-activity-item/tmp/acuity-phase-6a \
cargo run -p acuity
```

Agent reads the file directly for analysis.

- [x] **After Phase A:** Per-variant fields appear in log (parent_id/agent/model/title
  on session_updated lines). DEBUG lines show raw payload + parsed event.
  *Committed 369a0d1. Live-verified by reading `tmp/acuity.log` — all variants
  logging correctly with per-variant fields. Confirmed B2 + B4 defects.*
- [ ] **After B2 (dedup):** Run an opencode session. Consecutive
  `session_updated` events for the same session show collapsed — was 5 identical
  rows, now 1-2. Agent reads log to confirm. *Committed `db90d51` — pending live verify.*
- [ ] **After B4 (model):** `agent_turn_completed` lines show
  `model=Some("anthropic/claude-sonnet")` (or equivalent). `session_updated`
  lines still show `model=None` for subagents (expected — no model on
  session creation). Agent reads log to confirm. *Committed `3aeb835` — pending live verify.*
- [ ] **Wire-vs-deser delta (after B4):** DEBUG lines confirm `model` field
  present in raw payload (plugin sent it) matches `Some(...)` in `?event`
  (schema parsed it correctly). *Pending live verify.*

---

## Deferred (not in this plan)

- Curator ingest of `model` from `AgentTurnCompleted` (original step 6 of
  `slice6a-hardening.md`). Triggered when Phase B is live-verified.
- Slice 6b rendering.
- Sessions-table normalization.
