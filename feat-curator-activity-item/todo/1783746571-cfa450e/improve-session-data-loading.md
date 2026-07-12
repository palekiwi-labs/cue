---
status: closed
priority: low
refs:
- .cue/feat-curator-activity-item/spec/index.md
- .cue/feat-curator-activity-item/plan/1783746571-cfa450e/slice6c-two-pane-activity.md
---
# Improve session data loading

On startup, the curator fetches a flat window of events (capped at EVENT_CAP=2000)
regardless of session boundaries. This creates two problems:

1. **Per-event eviction**: the ring buffer evicts the oldest individual event
   (not the oldest complete session). A long session can crowd out multiple
   shorter sessions entirely.

2. **Startup gap**: sessions that existed before the curator started are not
   loaded until their next event arrives. There is no "fetch last N sessions"
   on connect.

## Proposed improvements

- **Per-session ring-buffer eviction**: instead of evicting oldest individual
  events, evict the oldest complete session (all its events at once). This keeps
  Pane 1's session list stable and avoids the "session disappears mid-scroll"
  edge case.
- **Startup N-session fetch**: on SSE connect, fetch the last N sessions' full
  event sets so the activity feed is populated immediately.

## Trigger

Action when: a second harness ('pi') is added (multi-harness sessions make the
per-event eviction confusion visible), or the flat layout becomes a bottleneck
for long-running sessions.
