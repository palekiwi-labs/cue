---
status: complete
---
## Foreword

Implements Slice 5 of `plan/1782644149-dab157e/curator-improvements.md`.

This slice is purely a **logic layer** — no rendering changes. It delivers:
1. A bug fix in `push_event` (project_dir set from all events, not just idle).
2. A new `activity.rs` module with the `ActivityItem<'a>` enum and the pure
   `build_activity_items` function, fully unit-tested.

Slice 6 owns the rendering rewrite and selection-identity work. This slice
does NOT touch `ui.rs`, `main.rs`, or the live render path.

**Branch:** `feat/curator-activity-item`
**Test exit:** `cargo test -p curator` green + `cargo clippy -p curator -- -D warnings` clean.

Design finalized via Opus review. Key decisions:
- Borrow-based `ActivityItem<'a>` — avoids cloning whole EventRecords.
- `HashMap<(&'a str, &'a str), usize>` for `turn_map` — no per-event String clone.
- Folded tool calls are suppressed (not Standalone) — Standalone is for true orphans.
- `turn_id: None` → `Standalone`, not `unwrap_or("")` (avoids empty-key collision).
- Header triggered by `(session_id, project_dir)` pair — mid-session project change
  gets a new header (intentional per spec).

---

## Steps

- [x] **1. Fix `push_event` in `crates/curator/src/app.rs`**

  a. Insert after the `last_seen` update, before the `match` arm:
     ```rust
     // Always reflect the ingest-time project_dir from EventRecord (set at
     // DB ingest from the event payload). Non-empty on all events since Slice 2.
     if !record.project_dir.is_empty() {
         entry.project_dir.clone_from(&record.project_dir);
     }
     ```

  b. Remove the now-redundant `entry.project_dir.clone_from(&ev.project_dir)`
     line from the `"session_idle"` match arm. `session_title` assignment stays.

  c. Add test `project_dir_set_from_non_idle_first_event` in `app.rs`:
     Seeds session with only `AgentTurnCompleted` (no idle),
     asserts `app.sessions["s1"].project_dir == "/home/pl/code"`.

  d. `cargo test -p curator` green. Commit: `fix: set project_dir from all events in push_event`.

- [x] **2. Create `crates/curator/src/activity.rs`**

  **2a. ActivityItem enum**

  ```rust
  pub enum ActivityItem<'a> {
      SessionHeader {
          session_id: &'a str,           // for renderer short-id fallback
          project_dir: &'a str,
          harness: &'a str,
          session_title: Option<&'a str>, // from sessions HashMap
      },
      Turn {
          agent_turn: &'a EventRecord,
          tool_calls: Vec<&'a EventRecord>, // empty = folded; non-empty = expanded
      },
      Standalone(&'a EventRecord),
  }
  ```

  **2b. build_activity_items signature**

  ```rust
  pub fn build_activity_items<'a>(
      events: &'a VecDeque<EventRecord>,
      sessions: &'a HashMap<String, SessionSummary>,
      fold_state: &HashSet<(String, String)>,
  ) -> Vec<ActivityItem<'a>>
  ```

  **2c. Algorithm (reverse-chrono)**

  ```
  init:
    items: Vec<ActivityItem<'a>>
    turn_map: HashMap<(&'a str, &'a str), usize>   // key borrows from events ('a)
    prev: Option<(String, String)>                  // (session_id, project_dir)

  for record in events.iter().rev():

    // Header injection — clone only on emit (not every event)
    let same = prev.as_ref().map_or(false, |(s, p)| {
        s == &record.session_id && p == &record.project_dir
    });
    if !same:
      let title = sessions.get(&record.session_id)
          .and_then(|s| s.session_title.as_deref());
      items.push(SessionHeader { session_id, project_dir, harness, session_title: title });
      prev = Some((record.session_id.clone(), record.project_dir.clone()));

    // Event dispatch
    match record.event_type.as_str():

      "agent_turn_completed":
        let Some(turn_id) = record.turn_id.as_deref() else {
            items.push(Standalone(record)); continue;
        };
        items.push(Turn { agent_turn: record, tool_calls: vec![] });
        turn_map.insert((&record.session_id, turn_id), items.len() - 1);

      "tool_call_requested" | "tool_call_completed":
        let Some(turn_id) = record.turn_id.as_deref() else {
            items.push(Standalone(record)); continue;
        };
        match turn_map.get(&(record.session_id.as_str(), turn_id)):
          Some(&idx) if fold_state.contains(&(record.session_id.clone(), turn_id.to_string())):
            // expanded — attach
            if let ActivityItem::Turn { ref mut tool_calls, .. } = items[idx]:
              tool_calls.push(record);
          Some(_):
            // folded — SUPPRESS (not Standalone)
          None:
            // orphan — parent evicted from ring buffer
            items.push(Standalone(record));

      _ :  // session_idle and unknowns
        items.push(Standalone(record));

  return items
  ```

  **2d. Tests (inline #[cfg(test)] in activity.rs)**

  | # | Name | What it pins |
  |---|------|--------------|
  | 1 | `two_sessions_same_turn_id_no_cross_contamination` | `(session_id, turn_id)` key prevents contamination |
  | 2 | `orphan_tool_call_renders_as_standalone` | Parent turn evicted → Standalone, no panic |
  | 3 | `tool_call_lower_seq_than_turn_still_groups` | Reverse-chrono: turn seen first → HashMap ready |
  | 4 | `project_dir_change_within_session_emits_new_header` | `prev` includes project_dir |
  | 5 | `empty_fold_state_all_turns_have_empty_tool_calls` | Default folded |
  | 6 | `fold_state_entry_expands_that_turn_only` | Only keyed turn populates tool_calls |
  | 7 | `folded_turn_tool_calls_suppressed_not_standalone` | Folded != orphan — different suppression paths |

  Helpers: `make_record(seq, session_id, turn_id, event_type, event)` with
  parameterised `turn_id` (not hardcoded "t1").

- [x] **3. Wire `mod activity;` into `crates/curator/src/main.rs`**

  Add `mod activity;` at the top alongside other module declarations.
  No other main.rs changes — `build_activity_items` is not called yet.

- [x] **4. Final verification**

  ```
  cargo test -p curator
  cargo clippy -p curator -- -D warnings
  ```

  All tests green. Commit: `feat(curator): ActivityItem enum and build_activity_items`.

---

## Out of scope (Slice 6)

- Adding `fold_state: HashSet<(String, String)>` to `App`
- Changing `sel_activity` / introducing selection identity
- Rewriting `render_activity` in `ui.rs`
- Calling `build_activity_items` from the run loop
