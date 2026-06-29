# Project Log

## [e19cf22] feat(acuity-schema): project_dir + harness added to all event types — commit e19cf22

Slice 1 of the curator improvements plan is code-complete. All four acuity-schema event structs now carry project_dir and harness. Two new AcuityEvent accessors (project_dir() and harness()) added following the session_id()/turn_id() pattern. All downstream test fixtures in acuity/src/db.rs, acuity/src/tests.rs, curator/src/app.rs, and curator/src/ui.rs updated. 138 workspace tests green; clippy -D warnings clean. Step 13 (types.ts regeneration) remains open — requires nix.

- **Found:** acuity/src/tests.rs carries integration-level JSON body constants (SESSION_IDLE_BODY etc.) that are separate from the db.rs unit-test fixtures — both needed updating
- **Found:** curator/src/ui.rs has its own parallel set of event struct constructors in test helpers, independent of app.rs — five files total needed fixture updates
- **Decided:** Placed project_dir and harness after session_id/turn_id and before optional/domain fields in each struct — consistent ordering across all four types
- **Decided:** Updated raw-JSON deserialization test strings to include new fields so they exercise the required-field path, not just the round-trip path
- **Open:** Step 13: regenerate types.ts via nix run .#update-types and commit to cue-plugins — requires nix shell

## [e19cf22] Slice 1 complete — schema-enrichment task closed

types.ts regenerated and committed to cue-plugins at ab481a4. All 5 acceptance criteria met. Task schema-enrichment.md marked complete. Executive plan marked complete.

- **Decided:** schema-enrichment task status set to complete; branch field retained for traceability

## [993efec] [993efec] feat(acuity): Slice 2 complete — DB layer and EventRecord

project_dir and harness propagated through the full persistence layer. SCHEMA_SQL gains two new NOT NULL columns and idx_events_project index. connect() now runs a PRAGMA table_info guard on startup — stale DB files without the new columns return Err with a clear message. insert_event and query_events_after updated. EventRecord in acuity-api carries the two new fields. 37/37 tests green, clippy clean.

- **Found:** tempfile crate was not yet in dev-dependencies — added at 3.x
- **Found:** PRAGMA table_info returns rows accessible by column name 'name' with sqlx Row trait in scope
- **Found:** All 5 existing db unit-test fixtures already had the new event fields from Slice 1 — no fixture updates needed in db.rs tests
- **Decided:** Placed project_dir and harness between turn_id and payload in both DDL and EventRecord — mirrors the event struct field order from Slice 1
- **Decided:** Column guard runs after SCHEMA_SQL in connect() so it catches both fresh-open and re-open of stale files
- **Decided:** Stale-file test uses tempfile::tempdir() to build a real on-disk old-schema DB, then calls connect() and asserts Err

## [16e01bb] [16e01bb] feat(acuity): Slice 3 complete — project_dir filter on GET /events

query_events_after gains a sixth project_dir: Option&lt;&amp;str&gt; parameter with an AND predicate added to the dynamic query builder, following the exact pattern of session_id and event_type. EventsQuery in main.rs gets the matching Option&lt;String&gt; field. SSE drain loop updated with None. New test query_project_dir_filter inserts two events with the same session but different project_dirs and asserts only the matching row is returned. 38/38 green, clippy clean.

- **Decided:** SSE drain loop passes None for project_dir — the stream is unfiltered by design; callers that want a filtered stream should use GET /events with pagination
- **Decided:** SESSION_IDLE_BODY_PD2 constant uses same session_id as SESSION_IDLE_BODY to verify the filter is keyed on project_dir and not session_id

## [86b1a48] [86b1a48] refactor(acuity): review-driven fixes applied

Code review from diff-reviewer-opus and diff-reviewer-glm produced 7 minor findings. All were applied in a single follow-up commit. Most importantly, the stale-DB guard was repositioned to run BEFORE SCHEMA_SQL — the test added by the review process exposed a real latent bug: SCHEMA_SQL contains CREATE INDEX ON events(project_dir) which would itself fail with a cryptic "no such column" error on a stale DB before the guard could fire.

- **Found:** Latent bug: stale-DB guard was placed AFTER sqlx::query(SCHEMA_SQL).execute() — but SCHEMA_SQL contains CREATE INDEX ON events(project_dir) which fails first on a stale DB with a cryptic error before the guard fires. The strengthened test assertion (checking error message content, not just is_err()) caught this immediately.
- **Found:** Strengthened stale-DB test confirmed the real error path required repositioning the PRAGMA check to BEFORE SCHEMA_SQL execution, with an empty-rows check to skip the guard on fresh DBs
- **Decided:** EventFilter struct introduced in db.rs — query_events_after now takes EventFilter instead of 3 positional Option params; SSE drain loop uses EventFilter::default()
- **Decided:** PRAGMA guard runs BEFORE SCHEMA_SQL with a rows.is_empty() fast-path for fresh DBs
- **Decided:** Error string deduplicated: built with format!() once, passed to both tracing::error!() and anyhow::anyhow!()
- **Decided:** Guard doc comment narrowed to 'detects absence of required columns by name' — does not claim type/nullability validation
- **Decided:** Test renamed: project_dir_column_exists_in_schema -> new_columns_exist_in_schema to reflect dual assertion
- **Decided:** query_project_dir_filter strengthened: unfiltered control assertion + reverse-direction check
- **Decided:** New test: query_project_dir_and_session_id_combined_filter confirms AND conjunction across multiple filter fields

## [dd6f86b] [dd6f86b] fix(curator): missed fixture constructors caught by QA

Manual QA triggered cargo test --workspace which caught three test fixture constructors in curator that were not updated during Slice 1. Fixed make_record() in app.rs and ui.rs (derive project_dir/harness from the event accessors), and record_json()/multiline test in sse.rs (add fields to inline JSON). Workspace now fully green.

- **Found:** Three curator test helpers were missed during Slice 1 fixture sweep: make_record() in app.rs, make_record() in ui.rs, record_json() in sse.rs — all lacked project_dir and harness fields on EventRecord
- **Found:** SSE multiline test had inline JSON split across two data: lines; the split point had to be moved to keep each fragment valid after adding the new fields
- **Decided:** Derived project_dir and harness in make_record() from the AcuityEvent accessors (event.project_dir(), event.harness()) rather than hardcoding — consistent with how other fields are populated

