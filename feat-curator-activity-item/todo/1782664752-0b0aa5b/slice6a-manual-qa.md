---
status: complete
priority: normal
---
# Slice 6a Manual QA Checklist

> **Completed** via live log file analysis against the hardened pipeline
> (post-f3b11f7/db90d51/3aeb835). The QA was performed by running an opencode
> session with a Task sub-agent (explore) and reading `tmp/acuity.log`.
> Evidence: `session_updated` dedup collapsed from 5+ to 2 per session;
> `agent_turn_completed` carries non-null `model` on every turn including
> the subagent (`google/gemini-3-flash-preview`); parent_id lineage captured
> correctly; title arrives before session_idle. See log entry [d590d61].
>
> Note: This checklist was written for the initial 6a baseline (3 commits)
> and uses the old default DB path. The hardening plan's verification
> checklist (slice6a-logging-and-plugin.md) covers the dedup and model
> checks against the hardened pipeline.

Manual end-to-end verification of the `SessionUpdated` collection layer.
The automated tests (schema 25/25, curator 47/47, workspace clippy clean,
tsc clean) are already green — these steps verify the **live pipeline**
(opencode plugin -> acuity server -> SQLite) produces correct lineage rows.

## Context

- **DB path:** `~/.local/share/acuity/events.db` (or `$ACUITY_DATA_DIR/acuity/events.db` if set)
- **Acuity server:** `cargo run -p acuity` from the cue repo root, listens on port 33222
- **Plugin:** loaded via `~/.config/opencode/opencode.json` (absolute path to `acuity-plugin.ts`)
- **Three commits to verify:** `7f4bac5` (schema), `1acf52f` (curator), `8555572` in cue-plugins (plugin handlers)

## Steps

### 1. Smoke test: post a SessionUpdated event directly via curl

Fast sanity check that the acuity server accepts the new event type and
persists it. Requires the acuity server running (step 2 below).

```bash
# Start the server in the background if not already running
cargo run -p acuity &

# Post a test SessionUpdated event
curl -s -X POST http://localhost:33223/events \
  -H "Content-Type: application/json" \
  -H "X-Acuity-Schema: 1" \
  -d '{
    "type": "session_updated",
    "session_id": "ses_smoke_test",
    "project_dir": "/tmp",
    "harness": "opencode",
    "parent_id": "ses_parent_123",
    "agent": "claude",
    "model": "anthropic/claude-sonnet",
    "title": "smoke test"
  }'

# Verify it landed
sqlite3 ~/.local/share/acuity/events.db \
  "SELECT event_type, session_id, project_dir, harness FROM events WHERE session_id = 'ses_smoke_test';"
```

- [x] curl returns 200
- [x] sqlite3 shows one row: `session_updated|ses_smoke_test|/tmp|opencode`
- [ ] payload column contains parent_id/agent/model/title (verify with the query in step 5)

### 2. Drop and recreate the acuity DB

Ensures a clean slate with no stale events from prior sessions.

```bash
# Stop the acuity server first (Ctrl+C or kill the background job)
# Then delete the DB file — the server recreates it on next startup
rm -f ~/.local/share/acuity/events.db

# Confirm it's gone
ls ~/.local/share/acuity/events.db 2>&1
# Expected: No such file or directory
```

- [ ] DB file deleted

### 3. Start the acuity server fresh

```bash
# From the cue repo root
cargo run -p acuity
# Watch the startup logs — confirm "listening on 0.0.0.0:33222" (or similar)
# and that the DB was created fresh
```

- [ ] Server starts without error
- [ ] DB file recreated: `ls ~/.local/share/acuity/events.db` exists again

### 4. Run an opencode session that spawns a sub-agent

Open a **new terminal** and run opencode. Ask it to use the Task tool,
which spawns a child session with `parentID` set.

```bash
# In a new terminal, from any project
opencode

# Then prompt something like:
#   "Use the Task tool to explore the repo structure and summarize it."
#
# This spawns a sub-agent session. The child session will fire:
#   session.created (carrying parentID = the main session's id)
#   session.updated (carrying the regenerated title + agent + model)
```

- [ ] Opencode session completes (main + at least one sub-agent)
- [ ] No errors in the acuity server logs (no "acuity rejected event")

### 5. Query the DB for SessionUpdated lineage rows

```bash
# All session_updated events with lineage unpacked from the payload JSON
sqlite3 ~/.local/share/acuity/events.db \
  "SELECT
     seq,
     session_id,
     json_extract(payload, '$.parent_id')  AS parent_id,
     json_extract(payload, '$.agent')       AS agent,
     json_extract(payload, '$.model')       AS model,
     json_extract(payload, '$.title')       AS title
   FROM events
   WHERE event_type = 'session_updated'
   ORDER BY seq;"
```

- [ ] At least 2+ `session_updated` rows exist (one per session: main + children)
- [ ] Sub-agent sessions show a non-null `parent_id` matching the parent session_id
- [ ] Primary (main) session shows `parent_id` = null
- [ ] `agent` and `model` columns are populated (non-null) for recent sessions
- [ ] `title` is populated (non-null) — this is the early title from session.updated

### 6. Verify the title row precedes the session_idle row

The key win of slice 6a: the title arrives via `session.updated` **before**
`session.idle`, so the activity feed can show a title without waiting for
the session to go idle.

```bash
# For a specific session, show the chronological event ordering
sqlite3 ~/.local/share/acuity/events.db \
  "SELECT
     seq,
     event_type,
     CASE event_type
       WHEN 'session_updated' THEN json_extract(payload, '$.title')
       WHEN 'session_idle'    THEN json_extract(payload, '$.session_title')
     END AS title_at_that_event
   FROM events
   WHERE session_id = (
     SELECT session_id FROM events
     WHERE event_type = 'session_updated'
     LIMIT 1
   )
   ORDER BY seq;"
```

- [ ] The `session_updated` row (with title) appears at a **lower seq** than the `session_idle` row
- [ ] The title value matches between the two (or session_updated has it first)

### 7. Verify the curator receives the lineage via SSE (optional)

If the curator TUI is running, confirm the `SessionSummary` in memory has
the lineage. This is hard to inspect directly (no debug view yet — that's
slice 6b), so this step is optional. The DB query in step 5 is the
authoritative check.

```bash
# Start the curator (it connects to acuity via SSE)
cargo run -p curator
# Navigate to the Activity view — sessions should appear
# (Rendering of title/agent/lineage headers is slice 6b, not yet done)
```

- [ ] Curator starts and connects to acuity (status bar shows "Connected")
- [ ] Events appear in the Activity view (lineage rendering is 6b, deferred)

## Cleanup

```bash
# Kill the acuity server background process when done
kill %1  # or find and kill the cargo run -p acuity process
```
