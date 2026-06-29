---
status: complete
---
## Foreword

Implements Slice 2 of `plan/1782644149-dab157e/curator-improvements.md`.

**Goal:** Propagate `project_dir` and `harness` from the schema layer (Slice 1)
through the SQLite persistence layer and the `acuity-api` query types so the
curator can read these fields from `EventRecord`.

**Branch:** `feat/curator-improvements-schema`

**Depends on:** Slice 1 complete (commit e19cf22). All four `AcuityEvent`
variants already carry `project_dir` and `harness`; `event.project_dir()` and
`event.harness()` accessors are available.

**Files in scope:**
- `crates/acuity-api/src/lib.rs` — add fields to `EventRecord`
- `crates/acuity/src/db.rs` — DDL, guard, insert, query
- `crates/acuity/Cargo.toml` — add `tempfile` dev-dep
- `crates/acuity/src/tests.rs` — extend field-check test

**Exit condition:** `cargo test -p acuity` green, `cargo clippy -p acuity
-- -D warnings` clean.

---

## Steps

- [ ] 1. `acuity-api/src/lib.rs`: add `pub project_dir: String` and
  `pub harness: String` to `EventRecord`, after `turn_id`. Update the doc
  comment for the struct to mention the new fields.

- [ ] 2. `crates/acuity/src/db.rs` `SCHEMA_SQL`: add
  `project_dir TEXT NOT NULL` and `harness TEXT NOT NULL` columns to
  the `events` table DDL (after `turn_id`). Add
  `CREATE INDEX IF NOT EXISTS idx_events_project ON events(project_dir);`
  at the end of the string.

- [ ] 3. `crates/acuity/src/db.rs` `connect()`: after the
  `sqlx::query(SCHEMA_SQL).execute(&pool).await?` call, add the startup
  column-mismatch guard:
  - Run `PRAGMA table_info(events)` and collect column names into a
    `HashSet<String>` using `use sqlx::Row as _;` and `row.get::<String,_>("name")`.
  - For each of `["project_dir", "harness"]`, if the column is absent:
    log an error (`tracing::error!`) with the message
    `"stale events.db detected — column `{col}` missing; delete the file and restart"`
    and return `Err(anyhow::anyhow!(...))`.

- [ ] 4. `crates/acuity/src/db.rs` `insert_event`: update the SQL string to
  include `project_dir` and `harness` in the column list and `?` placeholders.
  Add `.bind(event.project_dir())` and `.bind(event.harness())` in order after
  the existing `.bind(event.turn_id())`.

- [ ] 5. `crates/acuity/src/db.rs` `query_events_after`: update the SELECT
  string to include `project_dir, harness` after `turn_id`. Update the
  row-mapping closure to set `project_dir: row.get("project_dir")` and
  `harness: row.get("harness")`.

- [ ] 6. `crates/acuity/Cargo.toml`: add `tempfile = "3"` to
  `[dev-dependencies]`.

- [ ] 7. `crates/acuity/src/db.rs` tests: add
  `project_dir_column_exists_in_schema` — after `memory_pool()`, run
  `PRAGMA table_info(events)`, collect names, assert `"project_dir"` and
  `"harness"` are present.

- [ ] 8. `crates/acuity/src/db.rs` tests: add
  `connect_stale_db_returns_error` — create a `tempfile::tempdir()` path,
  manually open a SQLite file with only the old columns (no `project_dir` /
  `harness`), close that pool, then call `db::connect()` on the same path and
  assert `result.is_err()`.

- [ ] 9. `crates/acuity/src/tests.rs` `query_record_fields_correct`:
  add assertions `assert_eq!(rec.project_dir, "/home/me/project")` and
  `assert_eq!(rec.harness, "opencode")` after the existing field checks
  (SESSION_IDLE_BODY already carries these values).

- [ ] 10. Run `cargo test -p acuity` — must be green.

- [ ] 11. Run `cargo clippy -p acuity -p acuity-api -- -D warnings` — must be
  clean.

- [ ] 12. Commit:
  `feat(acuity): add project_dir + harness to DB layer and EventRecord`
