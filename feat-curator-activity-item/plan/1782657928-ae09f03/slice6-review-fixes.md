---
status: complete
---
## Foreword

Addresses findings from the dual code review (GLM + Opus) of `feat/curator-activity-item`,
refined by a follow-up Opus consultation on the plan itself.

Implements pre-Slice-6 fixes in `crates/curator/src/activity.rs` only.
No rendering changes. No `ui.rs`, `main.rs`, or `app.rs` code changes.

**Branch:** `feat/curator-activity-item`
**Test exit:** `cargo test -p curator` green + `cargo clippy -p curator -- -D warnings` clean.

### Findings addressed (in priority order)

| # | Severity | Finding |
|---|---|---|
| 1 | minor | `fold_state.contains()` allocates 2 `String`s per tool call per frame |
| 2 | minor | Duplicate `turn_id` attaches tool calls to older turn, not newer |
| 3 | minor | Missing tests: `turn_id: None` branches, empty deque, A→B→A re-entry, newest-wins |
| 4 | minor | `build()` test helper returns misleading `VecDeque::new()` with false doc-comment |
| 5 | nit | Append-only invariant on `turn_map` indices is undocumented |
| 6 | nit | `&record.session_id` vs `.as_str()` insert/lookup inconsistency |

### Opus consultation decisions

- **`or_insert_with` vs `or_insert`:** Use `or_insert_with` and move the `String`-allocating
  `expanded` computation *inside* the closure. On the duplicate-`turn_id` path the closure is
  not called, avoiding wasted heap allocations. Do NOT eagerly compute `expanded` and then use
  `or_insert_with` — that is the worst of both worlds.

- **Duplicate `turn_id` double-renders a second `Turn` item:** `or_insert` deduplicates only
  which Turn receives tool calls; the losing duplicate is still pushed as a rendered `Turn` item
  (line 110-113 runs before the map insert). The plan accepts this — double-rendering is the
  lesser UX harm and retries with the same `turn_id` are not expected in normal acuity usage.
  This known behavior is documented with a comment, not suppressed.

- **`SessionSummary.project_dir` = "latest non-empty":** Semantic decision, log-only. No code
  change. Captured in `cue log` after the commit.

- **`'static` Box::leak in test helpers:** Acceptable (bounded per-run leak). Update the now-
  false doc-comment to state the real model: "Leaks the deque for `'static`; acceptable in
  tests only."

- **`turn_id: None` defensive tests:** Both `agent_turn_completed` and `tool_call_*` schemas
  have non-optional `turn_id` in the payload; the `None` case is reached via `EventRecord.turn_id`
  column being `None` (malformed/legacy ingest data). Tests are constructed by passing `None` to
  `make_record`'s `turn_id` argument. This is the correct and only mechanism.

---

## Steps

- [x] **1+2. Fix hot-path allocation and duplicate `turn_id` (combined)**

  Change `turn_map` value type from `usize` to `(usize, bool)`.
  Update the `turn_map` declaration and its comment:

  ```rust
  // Maps (session_id, turn_id) -> (index into `items`, turn_is_expanded).
  // INVARIANT: `items` is append-only within this call. Stored indices are
  // positional and break if items is ever reordered or has elements removed.
  // Fold check is hoisted to turn-insertion time: one (String,String) alloc
  // per turn instead of one per tool call.
  let mut turn_map: HashMap<(&'a str, &'a str), (usize, bool)> = HashMap::new();
  ```

  At the `agent_turn_completed` arm, replace the existing `turn_map.insert(...)` with
  `entry().or_insert_with(...)`, moving the `String` allocation inside the closure so it
  is only paid for the first-seen (newest in reverse-chrono) occurrence:

  ```rust
  items.push(ActivityItem::Turn {
      agent_turn: record,
      tool_calls: vec![],
  });
  let idx = items.len() - 1;
  // or_insert_with: closure only runs for the first-seen (newest) turn_id.
  // Duplicate agent_turn_completed records (retries) still render as Turn
  // items but do not register in turn_map — their tool calls cannot attach.
  // This is intentional: newest turn owns the tool calls.
  turn_map
      .entry((record.session_id.as_str(), turn_id))
      .or_insert_with(|| {
          let expanded = fold_state
              .contains(&(record.session_id.clone(), turn_id.to_string()));
          (idx, expanded)
      });
  ```

  This simultaneously:
  - Fixes the hot-path allocation (zero alloc per tool call in `tool_call_*` arm)
  - Fixes duplicate turn_id (newest wins via first-entry semantics)
  - Normalizes the key to `.as_str()` (Step 4)
  - Adds the append-only invariant comment (Step 3)

  In the `tool_call_*` arm, destructure `(idx, expanded)` from the map:

  ```rust
  match turn_map.get(&(record.session_id.as_str(), turn_id)) {
      Some(&(idx, expanded)) => {
          if expanded {
              if let ActivityItem::Turn { ref mut tool_calls, .. } = items[idx] {
                  tool_calls.push(record);
              }
          }
          // else: turn is FOLDED — suppress (not Standalone).
      }
      None => {
          items.push(ActivityItem::Standalone(record));
      }
  }
  ```

- [x] **3. Refactor `build()` test helper**

  Replace the current `build()` (which returns a misleading `VecDeque::new()` with a false
  ownership comment) with a clean version returning `Vec<ActivityItem<'static>>` directly:

  ```rust
  /// Leak-based test helper. Builds items with an empty sessions map and empty
  /// fold_state (all turns folded). Leaks the deque; acceptable in tests only.
  fn build(events_vec: Vec<EventRecord>) -> Vec<ActivityItem<'static>> {
      let deque_ref: &'static VecDeque<EventRecord> =
          Box::leak(Box::new(events_vec.into_iter().collect()));
      let sessions_ref: &'static HashMap<String, SessionSummary> =
          Box::leak(Box::new(HashMap::new()));
      build_activity_items(deque_ref, sessions_ref, &HashSet::new())
  }
  ```

  Update all `let (_, items) = build(...)` call sites to `let items = build(...)`.

- [x] **4. Add missing tests**

  Add the following tests to the `#[cfg(test)]` module in `activity.rs`:

  a. **`agent_turn_with_null_turn_id_column_is_standalone`**
     Defensive test for `activity.rs:105-109`. The `AgentTurnCompleted` payload has a real
     `turn_id` but `make_record` is called with `turn_id: None` to set the column to `None`
     (simulating malformed/legacy ingest). The `let Some(...) else` arm triggers.
     Asserts: 1 header, 0 turns, 1 standalone.

  b. **`tool_call_with_null_turn_id_column_is_standalone`**
     Same construction for `tool_call_requested`. Column `turn_id = None`, payload has a value.
     Targets `activity.rs:119-123`.
     Asserts: 1 header, 0 turns, 1 standalone.

  c. **`empty_events_returns_empty_vec`**
     Passes an empty `VecDeque` to `build_activity_items`.
     Asserts: returned vec is empty (no panic, no header).

  d. **`project_dir_reentry_emits_third_header`**
     Events: `idle(1, s1, /a)`, `idle(2, s1, /b)`, `idle(3, s1, /a)`.
     In reverse-chrono, each event is a context change from the previous (A→B→A).
     Asserts: 3 headers, in order `/a`, `/b`, `/a`. Pins "compare against previous only"
     semantics so a future `HashSet`-of-seen-contexts "optimization" would break this test.

  e. **`duplicate_turn_id_newest_turn_receives_tool_calls`**
     Regression test for the `or_insert_with` fix. Two `agent_turn_completed` with the same
     `(session_id, turn_id)` — newer has higher seq. Expand the turn. Assert tool calls attach
     to the **newest** (higher-seq) Turn, not the older one.

- [x] **5. Verify and commit**

  ```
  cargo test -p curator
  cargo clippy -p curator -- -D warnings
  ```

  All green. Two commits (behavioral change separate from test/refactor):
  - `fix(curator): hoist fold check, fix duplicate turn_id newest-wins`
  - `test(curator): add edge-case tests, simplify build() helper`

---

## Out of scope (Slice 6)

- `fold_state: HashSet<(String, String)>` on `App` struct
- Calling `build_activity_items` from the run loop
- Rewriting `render_activity` in `ui.rs`
- Selection identity (`sel_activity`)
