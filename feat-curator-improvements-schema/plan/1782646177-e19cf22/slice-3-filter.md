---
status: complete
---
## Foreword

Implements Slice 3 of `plan/1782644149-dab157e/curator-improvements.md`.

**Goal:** Expose `project_dir` as an optional query filter on `GET /events`.
The dynamic query-builder pattern already exists for `session_id` and
`event_type`; this slice adds a third predicate using the same pattern.

**Branch:** `feat/curator-improvements-schema`

**Depends on:** Slice 2 complete. `project_dir` must be a real column in the
`events` table and present on `EventRecord` before the filter is testable.

**Files in scope:**
- `crates/acuity/src/db.rs` — add parameter to `query_events_after`
- `crates/acuity/src/main.rs` — add field to `EventsQuery`, thread through
- `crates/acuity/src/tests.rs` — new filter test

**Exit condition:** `cargo test -p acuity` green, `cargo clippy -p acuity
-- -D warnings` clean.

---

## Steps

- [ ] 1. `crates/acuity/src/db.rs` `query_events_after`: add
  `project_dir: Option<&str>` as the fifth parameter (after `event_type`).
  In the query builder, after the `event_type` predicate block, add:
  ```rust
  if let Some(pd) = project_dir {
      builder.push(" AND project_dir = ");
      builder.push_bind(pd);
  }
  ```

- [ ] 2. `crates/acuity/src/main.rs` `EventsQuery`: add
  `project_dir: Option<String>` field (after `event_type`). No `#[serde]`
  attribute needed — the field name is already the wire key.

- [ ] 3. `crates/acuity/src/main.rs` `query_events`: pass
  `params.project_dir.as_deref()` as the new fifth argument to
  `db::query_events_after`. Placement mirrors `params.event_type.as_deref()`.

- [ ] 4. `crates/acuity/src/main.rs` `sse_handler`: the SSE drain loop calls
  `db::query_events_after` with four positional args; update both call sites
  inside the loop to pass `None` as the fifth argument.

- [ ] 5. `crates/acuity/src/tests.rs`: add a constant:
  ```rust
  const SESSION_IDLE_BODY_PD2: &str = r#"{"type":"session_idle","session_id":"abc-123","project_dir":"/home/me/other","harness":"opencode","session_title":"other-proj"}"#;
  ```
  Then add test `query_project_dir_filter`: insert `SESSION_IDLE_BODY` (pd
  `/home/me/project`) and `SESSION_IDLE_BODY_PD2` (pd `/home/me/other`);
  GET with `project_dir=/home/me/other`; assert `page.events.len() == 1` and
  `page.events[0].project_dir == "/home/me/other"`.

- [ ] 6. Run `cargo test -p acuity` — must be green.

- [ ] 7. Run `cargo clippy -p acuity -- -D warnings` — must be clean.

- [ ] 8. Commit:
  `feat(acuity): add project_dir filter to GET /events`
