---
status: open
priority: low
---
# project_dir tripling — examine for deduplication

Opus noted during the curator improvements design review that after the schema
enrichment (Slice 1-2), `project_dir` is stored in three places per event:

1. The event struct field (in Rust; serialised into the `payload` column)
2. The dedicated `project_dir TEXT NOT NULL` SQLite column
3. The raw JSON in the `payload` column (as the event struct field)

This tripling is accepted for the prototype phase. The DB column is the source
of truth for server-side queries; `EventRecord.project_dir` is the hot-path
render copy; the copy inside `payload` is redundant dead weight.

Before shipping a non-prototype version, evaluate:
- Remove `project_dir` from the event structs and payload (requires schema-v3
  and plugin update) — payload becomes context-free; DB column is sole store
- Or accept the duplication as-is (storage is cheap; querying is clean)

Same concern applies to `harness`.
