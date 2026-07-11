use std::collections::{HashMap, HashSet, VecDeque};

use acuity_api::EventRecord;

use crate::app::SessionSummary;

/// A single renderable item in the Activity Feed.
///
/// Lifetime `'a` is tied to the `events` ring buffer and the `sessions` map
/// from which the items borrow. Items are render-pass-scoped — rebuild each
/// frame by calling [`build_activity_items`].
///
/// Fields are read by the renderer in Slice 6; the dead_code lint fires until
/// then because this module is compiled but not yet called from `ui.rs`.
#[allow(dead_code)]
pub enum ActivityItem<'a> {
    /// Group header emitted whenever the `(session_id, project_dir)` context
    /// changes in reverse-chrono order. `session_id` is included so the
    /// renderer can fall back to a short session-id when `project_dir` is
    /// empty.
    SessionHeader {
        session_id: &'a str,
        project_dir: &'a str,
        harness: &'a str,
        /// Resolved from the `sessions` map; `None` if no title is recorded.
        session_title: Option<&'a str>,
    },
    /// An `agent_turn_completed` event and its associated tool calls.
    ///
    /// `tool_calls` is empty when the turn is folded; non-empty when the turn
    /// is expanded (i.e. its `(session_id, turn_id)` key is in `fold_state`).
    /// The renderer checks `tool_calls.is_empty()` to decide the layout.
    Turn {
        agent_turn: &'a EventRecord,
        tool_calls: Vec<&'a EventRecord>,
    },
    /// Any event that is not grouped into a `Turn` — either a `session_idle`,
    /// an orphan tool call whose parent turn was evicted from the ring buffer,
    /// or any unrecognised event type.
    Standalone(&'a EventRecord),
}

/// Build the ordered list of [`ActivityItem`]s for the Activity Feed view.
///
/// Iterates `events` in reverse-chronological order (newest first).
/// Session-group headers are injected whenever the `(session_id, project_dir)`
/// context changes. `agent_turn_completed` events become [`ActivityItem::Turn`];
/// associated `tool_call_*` events are attached to their parent turn if (and
/// only if) the turn is expanded in `fold_state`. Tool calls whose parent turn
/// has been evicted from the ring buffer render as [`ActivityItem::Standalone`].
///
/// # Fold semantics
///
/// `fold_state` is the set of **expanded** `(session_id, turn_id)` keys.
/// An empty `fold_state` means all turns are folded (default). Toggling a
/// turn adds or removes its key. Folded turns always appear as `Turn` items
/// (so the summary row is visible), but their `tool_calls` vec is left empty.
/// Folded tool-call events are suppressed entirely — they do NOT fall through
/// to `Standalone`. Only orphan tool calls (missing parent) become `Standalone`.
///
/// # Lifetime
///
/// Both `events` and `sessions` are borrowed for `'a`. The returned vec holds
/// `&'a EventRecord` and `&'a str` slices into those collections.
///
/// # Unwired — Stage C owns the wiring
///
/// This function is **not called** by the Slice 6c two-pane renderer.
/// Stage-B Pane 2 (`render_events_pane` in `ui.rs`) uses a simpler per-session
/// flat filter instead. Stage C will wire this function into Pane 2 by adding a
/// `session_id: Option<&str>` filter at the top of the `events.iter().rev()`
/// loop; whether to suppress `SessionHeader` items in single-session mode is an
/// open design decision for stage C.
///
/// The `(session_id, project_dir)` header key is **degenerate**: `project_dir`
/// is workspace-constant (every event shares it), so the key collapses to
/// `session_id` alone. Stage C replaces this with `parent_id`-based nesting to
/// fold child sessions under their parent's turns.
///
/// Note: this entire module may be replaced or cleaned up in the stage-C PR if
/// the two-pane design renders the current grouping model obsolete.
#[allow(dead_code)] // called by the renderer in Slice 6
pub fn build_activity_items<'a>(
    events: &'a VecDeque<EventRecord>,
    sessions: &'a HashMap<String, SessionSummary>,
    fold_state: &HashSet<(String, String)>,
) -> Vec<ActivityItem<'a>> {
    let mut items: Vec<ActivityItem<'a>> = Vec::new();

    // Maps (session_id, turn_id) -> (index into `items`, turn_is_expanded).
    // Keys borrow from `events` ('a); no conflict with the &mut items access
    // because turn_map and items are disjoint allocations.
    // INVARIANT: `items` is append-only within this call. Stored indices are
    // positional and break if items is ever reordered or has elements removed.
    // Fold check is hoisted to turn-insertion time: one (String,String) alloc
    // per turn instead of one per tool call.
    let mut turn_map: HashMap<(&'a str, &'a str), (usize, bool)> = HashMap::new();

    // Tracks the (session_id, project_dir) pair of the last emitted header.
    // Cloned only when a new header is emitted — O(distinct sessions) total.
    let mut prev: Option<(String, String)> = None;

    for record in events.iter().rev() {
        // --- Session header injection ---
        // Emit a header whenever the (session_id, project_dir) context changes.
        // Comparing fields directly avoids constructing a tuple for every event.
        let same_ctx = prev
            .as_ref()
            .is_some_and(|(s, p)| s == &record.session_id && p == &record.project_dir);
        if !same_ctx {
            let session_title = sessions
                .get(&record.session_id)
                .and_then(|s| s.session_title.as_deref());
            items.push(ActivityItem::SessionHeader {
                session_id: &record.session_id,
                project_dir: &record.project_dir,
                harness: &record.harness,
                session_title,
            });
            prev = Some((record.session_id.clone(), record.project_dir.clone()));
        }

        // --- Event dispatch ---
        match record.event_type.as_str() {
            "agent_turn_completed" => {
                let Some(turn_id) = record.turn_id.as_deref() else {
                    // Malformed: agent_turn_completed without a turn_id.
                    items.push(ActivityItem::Standalone(record));
                    continue;
                };
                items.push(ActivityItem::Turn {
                    agent_turn: record,
                    tool_calls: vec![],
                });
                let idx = items.len() - 1;
                // or_insert_with: closure only runs for the first-seen (newest
                // in reverse-chrono) occurrence of this (session_id, turn_id).
                // Duplicate agent_turn_completed records (retries) still render
                // as Turn items but do not win the slot — their tool calls
                // cannot attach. This is intentional: newest turn owns the
                // tool calls. The String alloc for the fold check is inside the
                // closure so it is skipped entirely on the duplicate path.
                turn_map
                    .entry((record.session_id.as_str(), turn_id))
                    .or_insert_with(|| {
                        let expanded =
                            fold_state.contains(&(record.session_id.clone(), turn_id.to_string()));
                        (idx, expanded)
                    });
            }

            "tool_call_requested" | "tool_call_completed" => {
                let Some(turn_id) = record.turn_id.as_deref() else {
                    // Malformed: tool_call without a turn_id.
                    items.push(ActivityItem::Standalone(record));
                    continue;
                };
                match turn_map.get(&(record.session_id.as_str(), turn_id)) {
                    Some(&(idx, expanded)) => {
                        // Parent turn is in the ring buffer.
                        if expanded {
                            // Turn is expanded — attach tool call.
                            if let ActivityItem::Turn {
                                ref mut tool_calls, ..
                            } = items[idx]
                            {
                                tool_calls.push(record);
                            }
                        }
                        // else: turn is FOLDED — suppress (not Standalone).
                        // Standalone is reserved for true orphans (None arm).
                    }
                    None => {
                        // Orphan: parent turn was evicted from the ring buffer.
                        items.push(ActivityItem::Standalone(record));
                    }
                }
            }

            _ => {
                // session_idle and all unrecognised event types.
                items.push(ActivityItem::Standalone(record));
            }
        }
    }

    items
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use acuity_schema::{
        AcuityEvent, AgentTurnCompleted, SessionIdle, ToolCallCompleted, ToolCallRequested,
    };

    // --- Helpers ---

    fn make_record(
        seq: i64,
        session_id: &str,
        turn_id: Option<&str>,
        event: AcuityEvent,
    ) -> EventRecord {
        EventRecord {
            seq,
            received_at: format!("2026-01-01T00:00:{:02}Z", seq),
            event_type: event.event_type().to_string(),
            session_id: session_id.to_string(),
            turn_id: turn_id.map(str::to_string),
            project_dir: event.project_dir().to_string(),
            harness: event.harness().to_string(),
            payload: serde_json::to_string(&event).unwrap(),
        }
    }

    fn idle(seq: i64, session_id: &str, project_dir: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            None,
            AcuityEvent::SessionIdle(SessionIdle {
                session_id: session_id.to_string(),
                project_dir: project_dir.to_string(),
                harness: "opencode".to_string(),
                session_title: Some(format!("title-{session_id}")),
            }),
        )
    }

    fn agent_turn(seq: i64, session_id: &str, turn_id: &str, project_dir: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            Some(turn_id),
            AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                project_dir: project_dir.to_string(),
                harness: "opencode".to_string(),
                input_tokens: Some(10),
                output_tokens: Some(20),
                model: None,
            }),
        )
    }

    fn tool_req(seq: i64, session_id: &str, turn_id: &str, project_dir: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            Some(turn_id),
            AcuityEvent::ToolCallRequested(ToolCallRequested {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                project_dir: project_dir.to_string(),
                harness: "opencode".to_string(),
                tool_call_id: format!("c{seq}"),
                tool_name: "bash".to_string(),
                args: serde_json::Value::Null,
            }),
        )
    }

    fn tool_done(seq: i64, session_id: &str, turn_id: &str, project_dir: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            Some(turn_id),
            AcuityEvent::ToolCallCompleted(ToolCallCompleted {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
                project_dir: project_dir.to_string(),
                harness: "opencode".to_string(),
                tool_call_id: format!("c{seq}"),
                tool_name: "bash".to_string(),
                is_error: false,
                error_text: None,
            }),
        )
    }

    /// Build items from `events` (oldest-first) with an empty sessions map and
    /// empty fold_state (all turns folded).
    ///
    /// Leaks the deque and sessions map to obtain `'static` references for
    /// `ActivityItem<'static>`. The leaked memory is bounded per test run and
    /// reclaimed at process exit. Acceptable in tests only.
    fn build(events_vec: Vec<EventRecord>) -> Vec<ActivityItem<'static>> {
        let deque_ref: &'static VecDeque<EventRecord> =
            Box::leak(Box::new(events_vec.into_iter().collect()));
        let sessions_ref: &'static HashMap<String, SessionSummary> =
            Box::leak(Box::new(HashMap::new()));
        build_activity_items(deque_ref, sessions_ref, &HashSet::new())
    }

    /// Like `build` but with a non-empty fold_state.
    fn build_with_fold(
        events_vec: Vec<EventRecord>,
        fold_state: HashSet<(String, String)>,
    ) -> Vec<ActivityItem<'static>> {
        let deque_ref: &'static VecDeque<EventRecord> =
            Box::leak(Box::new(events_vec.into_iter().collect()));
        let sessions_ref: &'static HashMap<String, SessionSummary> =
            Box::leak(Box::new(HashMap::new()));
        build_activity_items(deque_ref, sessions_ref, &fold_state)
    }

    // Helper: count items of each kind.
    fn counts(items: &[ActivityItem<'_>]) -> (usize, usize, usize) {
        let headers = items
            .iter()
            .filter(|i| matches!(i, ActivityItem::SessionHeader { .. }))
            .count();
        let turns = items
            .iter()
            .filter(|i| matches!(i, ActivityItem::Turn { .. }))
            .count();
        let standalones = items
            .iter()
            .filter(|i| matches!(i, ActivityItem::Standalone(_)))
            .count();
        (headers, turns, standalones)
    }

    fn turn_tool_calls(item: &ActivityItem<'_>) -> usize {
        match item {
            ActivityItem::Turn { tool_calls, .. } => tool_calls.len(),
            _ => panic!("expected Turn"),
        }
    }

    // --- Test 1: two sessions, same turn_id — no cross-contamination ---

    #[test]
    fn two_sessions_same_turn_id_no_cross_contamination() {
        // Both sessions use turn_id "t1". Events in realistic order:
        // tool call arrives before agent_turn (tool calls have lower seqs).
        // Oldest to newest: s1-tool, s1-turn, s2-tool, s2-turn.
        // In reverse-chrono: s2-turn(4), s2-tool(3), s1-turn(2), s1-tool(1).
        let events = vec![
            tool_done(1, "s1", "t1", "/proj/s1"),
            agent_turn(2, "s1", "t1", "/proj/s1"),
            tool_done(3, "s2", "t1", "/proj/s2"),
            agent_turn(4, "s2", "t1", "/proj/s2"),
        ];

        // Expand both turns so tool_calls would be populated if grouped.
        let mut fold_state = HashSet::new();
        fold_state.insert(("s1".to_string(), "t1".to_string()));
        fold_state.insert(("s2".to_string(), "t1".to_string()));

        let items = build_with_fold(events, fold_state);

        // Expect: header(s2), turn(s2)+1tool, header(s1), turn(s1)+1tool.
        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 2, "one header per session");
        assert_eq!(turns, 2, "one Turn per session");
        assert_eq!(standalones, 0, "no orphans");

        // The s2 Turn comes first (reverse-chrono). Find it by session_id.
        let s2_turn_idx = items.iter().position(|i| {
            matches!(i, ActivityItem::Turn { agent_turn, .. }
                if agent_turn.session_id == "s2")
        });
        let s1_turn_idx = items.iter().position(|i| {
            matches!(i, ActivityItem::Turn { agent_turn, .. }
                if agent_turn.session_id == "s1")
        });
        assert!(s2_turn_idx.is_some() && s1_turn_idx.is_some());

        // Each Turn has exactly 1 tool call — not 2 (no cross-contamination).
        assert_eq!(turn_tool_calls(&items[s2_turn_idx.unwrap()]), 1);
        assert_eq!(turn_tool_calls(&items[s1_turn_idx.unwrap()]), 1);
    }

    // --- Test 2: orphan tool call renders as Standalone ---

    #[test]
    fn orphan_tool_call_renders_as_standalone() {
        // A tool call whose parent agent_turn is NOT in the events list
        // (simulating eviction from the ring buffer).
        let events = vec![
            // No agent_turn for session "s1", turn "t1" in the ring buffer.
            tool_done(2, "s1", "t1", "/proj"),
        ];
        let items = build(events);

        // Expect: header, standalone (the orphan tool call).
        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 0, "no Turn — parent was evicted");
        assert_eq!(standalones, 1, "orphan tool call is Standalone");

        if let ActivityItem::Standalone(r) = &items[1] {
            assert_eq!(r.event_type, "tool_call_completed");
        } else {
            panic!("expected Standalone at index 1");
        }
    }

    // --- Test 3: tool call with lower seq than its turn still groups ---

    #[test]
    fn tool_call_lower_seq_than_turn_still_groups() {
        // Unusual but possible: tool_call has a lower seq than its turn.
        // In reverse-chrono iteration the turn (higher seq) is seen FIRST,
        // so the HashMap entry already exists when the tool call is processed.
        let events = vec![
            tool_req(1, "s1", "t1", "/proj"),   // seq 1 — lower
            agent_turn(2, "s1", "t1", "/proj"), // seq 2 — higher
        ];

        let mut fold_state = HashSet::new();
        fold_state.insert(("s1".to_string(), "t1".to_string()));

        let items = build_with_fold(events, fold_state);

        let (_, turns, standalones) = counts(&items);
        assert_eq!(turns, 1, "one Turn");
        assert_eq!(standalones, 0, "no orphan — tool call grouped into Turn");
        assert_eq!(turn_tool_calls(&items[1]), 1, "tool call attached");
    }

    // --- Test 4: project_dir change within same session emits new header ---

    #[test]
    fn project_dir_change_within_session_emits_new_header() {
        // Same session, but the project_dir changes between events.
        // In reverse-chrono: event with /proj/b is seen first.
        let events = vec![idle(1, "s1", "/proj/a"), idle(2, "s1", "/proj/b")];
        let items = build(events);

        // Two headers: one for /proj/b (seen first), one for /proj/a.
        let headers: Vec<_> = items
            .iter()
            .filter_map(|i| {
                if let ActivityItem::SessionHeader { project_dir, .. } = i {
                    Some(*project_dir)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(headers.len(), 2, "two headers for two project_dirs");
        assert_eq!(headers[0], "/proj/b", "newest project_dir first");
        assert_eq!(headers[1], "/proj/a");
    }

    // --- Test 5: empty fold_state — all turns folded, tool_calls empty ---

    #[test]
    fn empty_fold_state_all_turns_have_empty_tool_calls() {
        // Realistic ordering: tool calls arrive before agent_turn (lower seqs).
        // Reverse-chrono: agent_turn(3), tool_done(2), tool_req(1).
        let events = vec![
            tool_req(1, "s1", "t1", "/proj"),
            tool_done(2, "s1", "t1", "/proj"),
            agent_turn(3, "s1", "t1", "/proj"),
        ];
        // build() uses an empty fold_state.
        let items = build(events);

        let (_, turns, standalones) = counts(&items);
        assert_eq!(turns, 1, "one Turn");
        assert_eq!(standalones, 0, "tool calls are suppressed, not Standalone");
        assert_eq!(
            turn_tool_calls(&items[1]),
            0,
            "fold_state empty -> tool_calls not populated"
        );
    }

    // --- Test 6: fold_state entry expands that turn only ---

    #[test]
    fn fold_state_entry_expands_that_turn_only() {
        // Realistic ordering: tool calls before agent_turn.
        // Reverse-chrono: agent_turn(t2,4), tool_done(t2,3), agent_turn(t1,2), tool_done(t1,1).
        let events = vec![
            tool_done(1, "s1", "t1", "/proj"),
            agent_turn(2, "s1", "t1", "/proj"),
            tool_done(3, "s1", "t2", "/proj"),
            agent_turn(4, "s1", "t2", "/proj"),
        ];

        // Expand only t2.
        let mut fold_state = HashSet::new();
        fold_state.insert(("s1".to_string(), "t2".to_string()));

        let items = build_with_fold(events, fold_state);

        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 2, "both turns present");
        assert_eq!(standalones, 0);

        // In reverse-chrono order: t2 (seq 3) is seen before t1 (seq 1).
        // items: [header, turn(t2)+1tool, turn(t1)+0tools]
        let t2_idx = items
            .iter()
            .position(|i| {
                matches!(i, ActivityItem::Turn { agent_turn, .. }
                if agent_turn.turn_id.as_deref() == Some("t2"))
            })
            .unwrap();
        let t1_idx = items
            .iter()
            .position(|i| {
                matches!(i, ActivityItem::Turn { agent_turn, .. }
                if agent_turn.turn_id.as_deref() == Some("t1"))
            })
            .unwrap();

        assert_eq!(turn_tool_calls(&items[t2_idx]), 1, "t2 expanded");
        assert_eq!(turn_tool_calls(&items[t1_idx]), 0, "t1 folded");
    }

    // --- Test 7: folded turn suppresses tool calls (not Standalone) ---

    #[test]
    fn folded_turn_tool_calls_suppressed_not_standalone() {
        // Folded turn (not in fold_state) — its tool calls must be suppressed,
        // NOT rendered as Standalone. Standalone is reserved for orphans.
        // Realistic ordering: tool calls arrive before agent_turn.
        // Reverse-chrono: agent_turn(3), tool_done(2), tool_req(1).
        let events = vec![
            tool_req(1, "s1", "t1", "/proj"),
            tool_done(2, "s1", "t1", "/proj"),
            agent_turn(3, "s1", "t1", "/proj"),
        ];
        // Empty fold_state — turn is folded.
        let items = build(events);

        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 1, "Turn is present (folded, but summary visible)");
        // Key assertion: 0 Standalones — the tool calls were suppressed,
        // not promoted to Standalone. Compare with test 2 (orphan → Standalone).
        assert_eq!(
            standalones, 0,
            "folded tool calls suppressed, not Standalone"
        );
        assert_eq!(
            turn_tool_calls(&items[1]),
            0,
            "tool_calls empty when folded"
        );
    }

    // --- Test 8: agent_turn_completed with None turn_id column → Standalone ---

    #[test]
    fn agent_turn_with_null_turn_id_column_is_standalone() {
        // Defensive test for the `let Some(turn_id) = record.turn_id.as_deref()
        // else { Standalone }` arm. The AgentTurnCompleted payload carries a real
        // turn_id, but make_record is called with turn_id: None so the persisted
        // EventRecord.turn_id column is None (malformed / legacy ingest data).
        let events = vec![make_record(
            1,
            "s1",
            None, // column None despite payload having a turn_id
            AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/proj".to_string(),
                harness: "opencode".to_string(),
                input_tokens: Some(10),
                output_tokens: Some(20),
                model: None,
            }),
        )];
        let items = build(events);

        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 0, "no Turn — column turn_id is None");
        assert_eq!(standalones, 1, "malformed turn becomes Standalone");
    }

    // --- Test 9: tool_call with None turn_id column → Standalone ---

    #[test]
    fn tool_call_with_null_turn_id_column_is_standalone() {
        // Same pattern as test 8 but for the tool_call_* arm. Column turn_id
        // is None even though the ToolCallRequested payload has a turn_id.
        let events = vec![make_record(
            1,
            "s1",
            None, // column None
            AcuityEvent::ToolCallRequested(ToolCallRequested {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/proj".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                args: serde_json::Value::Null,
            }),
        )];
        let items = build(events);

        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 0, "no Turn");
        assert_eq!(standalones, 1, "malformed tool call becomes Standalone");
        if let ActivityItem::Standalone(r) = &items[1] {
            assert_eq!(r.event_type, "tool_call_requested");
        } else {
            panic!("expected Standalone at index 1");
        }
    }

    // --- Test 10: empty events deque returns empty vec ---

    #[test]
    fn empty_events_returns_empty_vec() {
        let items = build(vec![]);
        assert!(items.is_empty(), "empty deque produces empty item list");
    }

    // --- Test 11: project_dir re-entry within same session emits third header ---

    #[test]
    fn project_dir_reentry_emits_third_header() {
        // Chrono: /a, /b, /a — each is a context change from the previous in
        // reverse-chrono order. Pins "compare against prev only" semantics: a
        // future HashSet-of-seen-contexts optimisation would break this test.
        let events = vec![
            idle(1, "s1", "/proj/a"),
            idle(2, "s1", "/proj/b"),
            idle(3, "s1", "/proj/a"),
        ];
        let items = build(events);

        let headers: Vec<_> = items
            .iter()
            .filter_map(|i| {
                if let ActivityItem::SessionHeader { project_dir, .. } = i {
                    Some(*project_dir)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            headers.len(),
            3,
            "re-entering a prior project_dir emits a fresh header"
        );
        assert_eq!(headers[0], "/proj/a", "newest first (seq 3)");
        assert_eq!(headers[1], "/proj/b");
        assert_eq!(headers[2], "/proj/a");
    }

    // --- Test 12: duplicate turn_id — newest turn receives tool calls ---

    #[test]
    fn duplicate_turn_id_newest_turn_receives_tool_calls() {
        // Two agent_turn_completed events share the same (session_id, turn_id).
        // The newer one (higher seq) is processed first in reverse-chrono order
        // and wins the turn_map slot. Tool calls must attach to it.
        //
        // Chrono order (oldest first):
        //   tool(1) - tool call for the turn
        //   turn(2) - older duplicate agent_turn_completed
        //   turn(3) - newer duplicate agent_turn_completed (seq 3 > seq 2)
        //
        // Reverse-chrono: turn(3), turn(2), tool(1)
        let events = vec![
            tool_done(1, "s1", "t1", "/proj"),
            agent_turn(2, "s1", "t1", "/proj"), // older duplicate
            agent_turn(3, "s1", "t1", "/proj"), // newer duplicate — should win
        ];

        let mut fold_state = HashSet::new();
        fold_state.insert(("s1".to_string(), "t1".to_string()));

        let items = build_with_fold(events, fold_state);

        // Two Turn items are rendered (both duplicates push a Turn), but only the
        // newer one (seen first in reverse-chrono) is in turn_map and can receive
        // tool calls.
        let (headers, turns, standalones) = counts(&items);
        assert_eq!(headers, 1);
        assert_eq!(turns, 2, "both duplicate turns render");
        assert_eq!(standalones, 0, "tool call is not orphaned");

        // The newer turn (seq 3) is first in the output (reverse-chrono).
        // Find it by checking which Turn has tool calls attached.
        let newer_turn_idx = items
            .iter()
            .position(|i| {
                matches!(i, ActivityItem::Turn { agent_turn, .. }
                    if agent_turn.seq == 3)
            })
            .unwrap();
        let older_turn_idx = items
            .iter()
            .position(|i| {
                matches!(i, ActivityItem::Turn { agent_turn, .. }
                    if agent_turn.seq == 2)
            })
            .unwrap();

        assert_eq!(
            turn_tool_calls(&items[newer_turn_idx]),
            1,
            "newer turn owns the tool call"
        );
        assert_eq!(
            turn_tool_calls(&items[older_turn_idx]),
            0,
            "older duplicate turn has no tool calls"
        );
    }
}
