---
title: Improve session data loading (per-session eviction + startup fetch)
status: open
priority: low
refs:
- .cue/feat-curator-activity-item/todo/1783746571-cfa450e/improve-session-data-loading.md
- .cue/master/spec/index.md
---
# Improve session data loading (per-session eviction + startup fetch)

On startup, the curator fetches a flat window of events (capped at
EVENT_CAP=2000) regardless of session boundaries. This creates two problems:

1. **Per-event eviction**: the ring buffer evicts the oldest individual event
   (not the oldest complete session). A long session can crowd out multiple
   shorter sessions entirely.
2. **Startup gap**: sessions that existed before the curator started are not
   loaded until their next event arrives. There is no "fetch last N sessions"
   on connect.

## Source

- Original todo: `.cue/feat-curator-activity-item/todo/1783746571-cfa450e/improve-session-data-loading.md`
- Spec: `.cue/master/spec/index.md`

## Trigger

Action when:
- A second harness ('pi') is added (multi-harness sessions make the per-event
  eviction confusion visible), OR
- The flat layout becomes a bottleneck for long-running sessions.

## Proposed improvements

- **Per-session ring-buffer eviction**: instead of evicting oldest individual
  events, evict the oldest complete session (all its events at once). Keeps
  Pane 1's session list stable and avoids the "session disappears mid-scroll"
  edge case.
- **Startup N-session fetch**: on SSE connect, fetch the last N sessions' full
  event sets so the activity feed is populated immediately.

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---------------------|-----------|----------|
| 1 | Ring buffer evicts oldest complete session, not individual events | unit test | |
| 2 | Curator fetches last N sessions' events on SSE connect | manual QA (feed populated on launch) | |
| 3 | Session does not disappear mid-scroll under event pressure | manual QA | |
| 4 | `cargo test --workspace` green | test run | |
| 5 | `cargo clippy --workspace -- -D warnings` clean | clippy run | |