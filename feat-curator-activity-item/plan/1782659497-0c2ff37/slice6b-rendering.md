---
status: complete
---
## Foreword

Fleshed out from the intentionally-light draft after 6a live data confirmed the
collection layer. Consumes the 6a `SessionSummary` fields (title, model,
parent_id, harness) to make the curator activity feed legible — *without*
surfacing agent, session-level model, or lineage text in headers (full rationale
in `spec/log.md`, "Slice 6b design locked" entry). Retains the flat reverse-chrono
layout; stage C owns the nested `parent_id` tree.

Three defects fixed:
1. **8-char front-truncation** (`ui.rs:214`) collides opencode session ids (they
   share a per-run prefix). Replaced with a title-or-id-suffix label.
2. **Headers show no 6a data** — now show title (or a dim id placeholder until
   the title arrives via `session_updated`).
3. **Turn rows hide the per-turn model** — now appended from the
   `AgentTurnCompleted` payload.

Plus: `session_updated` rows are hidden (their payload is already absorbed into
`SessionSummary` before render), and the selection model is hardened to index the
*filtered* set (mirrors `render_diagnostics`, `ui.rs:241-311`).

**Branch:** `feat/curator-activity-item`
**Test exit:** `cargo test -p curator` green + `cargo clippy -p curator -- -D warnings` clean.

### Design decisions (locked)

- **Header = title | dim id-suffix placeholder only.** No agent (belongs in
  diagnostics — a plan agent calling a mutating tool is the flag-worthy signal),
  no session-level model (misleading — models change per turn), no parent-id text
  (the stage-C tree shows lineage visually).
- **Per-turn model on turn rows** (from `AgentTurnCompleted.model`); appended as
  ` · {model}` when `Some`, omitted cleanly when `None`.
- **Harness + project in the block title once** (`Activity Feed · opencode / cue`),
  not per header — constant today; the `activity_block_title` helper makes
  per-header a one-line change if a second harness lands.
- **Hide `session_updated` rows.** They populate `SessionSummary` synchronously
  in `push_event` (`app.rs:230-250`); by render time their content is already in
  the header.
- **Selection indexes the filtered set**, mirroring Diagnostics
  (`ui.rs:241-311`, `app.rs:284-289`). Do NOT skip-on-render while keeping
  `sel_activity` over all events — that makes the highlight vanish/jitter when it
  lands on a hidden index.
- **event_type column left as-is.** Collapse deferred to observe in practice.
- **`build_activity_items` stays unwired.** Its `(session_id, project_dir)` header
  key is degenerate (project_dir workspace-constant); stage C rewrites it with
  `parent_id`-based nesting.

### Steps

- [x] **1. [PURE] `session_label` helper (`ui.rs`).** Commit `81f1853`.
  Extract `pub(crate) fn session_label(summary: Option<&SessionSummary>, session_id: &str) -> (String, bool)` — returns `(text, is_placeholder)`. Non-empty title → `(title, false)`; else an id *suffix* (last ~8 chars, boundary-guarded `str::get` with `unwrap_or` fallback, prefixed with `…`) → `(suffix, true)`. Red/green: title-present, title-None, title-empty-string, and a **regression test** that two ids sharing a prefix yield distinct labels (pins the `ui.rs:214` bug fix).

- [x] **2. [PURE] append per-turn model in `event_summary` (`ui.rs:333-343`).** Commit `9295f21`.
  In the `agent_turn_completed` arm, append ` · {model}` when `ev.model` is `Some`; omit cleanly (no dangling separator) when `None`. Red/green: `Some("anthropic/claude-sonnet")` includes it; `None` omits it. Update the existing `event_summary_agent_turn_completed` test (`ui.rs:434-448`, currently `model: None`).

- [x] **3. [PURE] `is_hidden_in_activity` + `App::activity_len` (`ui.rs`, `app.rs`).** Commit `d5ae36c`.
  `pub(crate) fn is_hidden_in_activity(et: &str) -> bool` — true for `"session_updated"` only (mirrors `is_diagnostic`, `ui.rs:374`). `App::activity_len()` counts non-hidden events (mirrors `diagnostics_len`, `app.rs:284-289`). Red/green including a session_updated-only feed → `activity_len() == 0`.

- [x] **4. [PURE] fix `scroll_down_activity` clamp (`app.rs:359-364`).** Commit `9b9be0e`.
  Change the bound from `self.events.len()` to `self.activity_len()`. **Correctness keystone — do before wiring.** Red test: with N events of which K are hidden, scrolling down past the end clamps at `activity_len()-1`, never past the last visible row. (Also pays down a latent bug: the visual list is longer than `events` because headers are injected.)

- [x] **5. [PURE] `activity_block_title` helper (`ui.rs`).** Commit `4dfef13`.
  `fn activity_block_title(app: &App) -> String` — `" Activity Feed · <harness> / <project-basename> "` when a session is present, else `" Activity Feed "`. Harness/project from the first available `SessionSummary`. Red/green: empty sessions → plain; one session → composed.

- [x] **6. [WIRE] rebuild `render_activity` (`ui.rs:152-210`).** Commit `cfa450e`.
  Integrate all helpers: (a) iterate the **filtered** set (`!is_hidden_in_activity`), driving both header injection and the `sel → visual` mapping off the filtered enumeration; (b) header from `session_label` with conditional style — dim (`DarkGray`) when `is_placeholder`, brighter/bold when title present (so the title-flip is a visible verification signal); (c) block title from step 5; (d) per-turn model already in `event_summary` from step 2; (e) render-time `sel_activity.min(activity_len()-1)` clamp + empty-list guard selecting `None` (copy `ui.rs:305-306`); (f) remove the old `session_header` fn (`ui.rs:213-234`). Manual visual verification.

- [x] **7. [MANUAL] live verify.** (pending user QA)
  Run the pipeline. Confirm: (a) prefix-sharing sessions now render distinct headers; (b) header flips dim-id → bright-title within seconds; (c) `session_updated` rows gone; (d) `j`/`k` never makes the highlight vanish or jitter (the drift test); (e) turn rows show per-turn model, and switching models mid-session shows different models on adjacent turns; (f) block title shows harness/project once.

- [x] **8. [PURE] `activity.rs` doc-comment.** Commit `cfa450e`.
  Extend the note at `activity.rs:16-27` to state explicitly that `build_activity_items` is unwired, the `(session_id, project_dir)` header key is degenerate (project_dir workspace-constant), and stage C rewrites with `parent_id`-based nesting. No code change.

### Out of scope

- Stage C: nested tree, folding child sessions under parent turns, the `build_activity_items` rewrite with `parent_id`-based nesting.
- event_type column collapse — deferred to observe in practice.
- Acuity server log trim (standalone todo).
- Sessions-table normalization (todo `normalize-events-sessions-table.md`).
