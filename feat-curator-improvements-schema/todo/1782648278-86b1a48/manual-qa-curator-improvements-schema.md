---
status: open
priority: high
---
# Manual QA — feat/curator-improvements-schema

Pre-merge QA checklist for Slices 1–3 of the curator improvements plan.
Run all steps in order. All gates must pass before merging.

## Branch

`feat/curator-improvements-schema`

## Steps

### 1. Automated gates

```bash
# Full workspace test suite
cargo test --workspace

# Clippy — zero warnings allowed
cargo clippy --workspace -- -D warnings
```

### 2. Stale-DB guard smoke test

```bash
# Create an old-schema DB (no project_dir / harness columns)
sqlite3 /tmp/stale-events.db "CREATE TABLE events (seq INTEGER PRIMARY KEY AUTOINCREMENT, received_at TEXT NOT NULL, event_type TEXT NOT NULL, session_id TEXT NOT NULL, turn_id TEXT, payload TEXT NOT NULL);"

# Point the server at it — must fail with a clear error message
# ACUITY_DATA_DIR controls the base dir; DB lives at $ACUITY_DATA_DIR/acuity/events.db
mkdir -p /tmp/stale-data/acuity
cp /tmp/stale-events.db /tmp/stale-data/acuity/events.db
ACUITY_DATA_DIR=/tmp/stale-data cargo run -p acuity 2>&1 | grep "stale events.db"
```

### 3. Fresh-server startup smoke test

```bash
# Start against a fresh DB — must start cleanly
rm -rf /tmp/fresh-data
mkdir -p /tmp/fresh-data
ACUITY_DATA_DIR=/tmp/fresh-data cargo run -p acuity &
sleep 1
curl -s http://localhost:33222/events | jq .
```

### 4. project_dir filter smoke test

```bash
# Insert a test row directly into the DB
sqlite3 /tmp/acuity-phase-1-3/acuity/events.db "INSERT INTO events (received_at, event_type, session_id, turn_id, project_dir, harness, payload) VALUES (datetime('now'), 'session_idle', 'test-sess', null, '/home/me/project', 'opencode', '{}');"

# Filter returns only matching rows
curl -s 'http://localhost:33223/events?project_dir=/home/me/project' | jq '.[].project_dir'

# Filter returns zero rows for non-matching project_dir
curl -s 'http://localhost:33223/events?project_dir=/other/project' | jq 'length'
```

## Merge Checklist

- [x] `cargo test --workspace` green
- [x] `cargo clippy --workspace -- -D warnings` clean
- [x] Stale-DB guard prints expected error and exits
- [x] Fresh server starts without errors
- [x] `GET /events?project_dir=...` returns only matching rows
- [x] `GET /events?project_dir=...` returns 0 rows for non-matching value
