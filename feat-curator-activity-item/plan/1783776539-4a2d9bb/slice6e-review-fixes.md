---
status: complete
refs:
- .cue/feat-curator-activity-item/tmp/1783776539-4a2d9bb/branch.diff
- .cue/feat-curator-activity-item/spec/index.md
- .cue/feat-curator-activity-item/log.md
---
---
status: complete
---
## Foreword

Addresses the findings from the dual code review (Opus + Flash) of the
`feat/curator-activity-item` branch diff (saved at
`.cue/feat-curator-activity-item/tmp/1783776539-4a2d9bb/branch.diff`).

The two reviewers converged on two pre-merge blockers and several
performance-relevant minors. This plan fixes the blockers and the perf
minors in three focused commits.

### Scope

**In scope (fixing):**
1. **#1 (Major, correctness):** `sel_activity` goes stale when the selected
   session's events are partially evicted from the ring buffer. The renderer
   masks it via `.min(len-1)`, but the stored index is stale — user must
   press `k` many times before the highlight visibly moves.
2. **#2 (Major, perf):** `session_unique_agents` / `session_unique_models`
   do a full ring-buffer scan + `serde_json::from_str` per matching event,
   every frame. Will stutter once the buffer fills (EVENT_CAP = 2000).
3. **O-5 (Minor, correctness):** `session_unique_models` has no
   `SessionSummary.model` fallback — asymmetric with agents.
4. **O-6 (Minor, perf):** `session_event_len` is O(N) scan, called
   per-session in `sorted_sessions` (O(N x CAP)) on every SSE message.
5. **O-2 (Nit, doc):** Info block doc says height 10 (8 data + 2 border);
   actually 11 (9 data + 2 border).
6. **O-4 (Nit, perf):** `Local::now().date_naive()` recomputed per row in
   `format_datetime`; two rows across midnight could disagree.
7. **O-3 (Nit, dead code):** `render_sessions_pane` hardcodes
   `let is_active = true;` then branches on it — else arms permanently dead.

**Out of scope (deliberately deferred):**
- `activity.rs` / `activity_len` dead code — tracked debt for stage C
  (nested tree). Well-documented, leaving as-is.
- F-1 (hardcoded UTF-8 char constants) — purely stylistic.
- F-2 (`session_label` title clone per frame) — n < 20 sessions, negligible.
- O-7 (redundant harness re-clone in session_updated arm) — defense in
  depth, one clone of a short string on deduped events.
- O-1 (Split -> DetailFull no sel_activity reset) — transitively resolved
  by the #1 clamp fix (entering DetailFull always has a valid clamped index).

### Design decisions

- **Caches go on `SessionSummary`**, not on `App`. SessionSummary is
  explicitly designed to "survive even when old events are evicted from the
  ring buffer" (app.rs:88-91). Caching `visible_event_count`,
  `unique_agents`, `unique_models` there is consistent with the existing
  design and makes all reads O(1).
- **`visible_event_count` is maintained in `push_event`**: increment on
  append (if non-hidden), decrement on eviction (if the evicted event was
  non-hidden). Uses `saturating_sub` to guard against any drift.
- **`unique_agents` / `unique_models` are `Vec<String>`** (first-seen
  order, deduped via `.contains()`). Small n (1-5 per session in practice).
  Accumulated in the existing `session_updated` / `agent_turn_completed`
  match arms — no extra deserialization.
- **Unique lists survive eviction** (better than current behavior, which
  falls back to the single last-writer-wins `SessionSummary.agent` when
  ring-buffer events are evicted).
- **`session_unique_agents` / `session_unique_models` in ui.rs** become
  thin readers of the cached Vecs — no more ring-buffer scan, no more
  serde. This also fixes O-5 (both now source from summary symmetrically).

### Steps

- [x] **1. fix(curator): clamp sel_activity on ring-buffer eviction (TDD)**
  Commit c6d3932.

- [x] **2. perf(curator): cache per-session aggregates on SessionSummary (TDD)**
  Commit 61ee148.

- [x] **3. style(curator): hoist frame clock, inline active session styles**
  Commit 1839275.

- [x] **4. Final verification**
  - `cargo test --workspace` — all suites green (106 curator tests)
  - `cargo clippy --workspace -- -D warnings` — clean
  - Consultant-opus review: **SHIP** (all three commits correct, cache
    invariant proven airtight, no new issues).
  - Belt-and-suspenders test added: commit 92605bc (fully-evicted-then-reused).
