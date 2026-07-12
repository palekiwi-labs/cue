---
status: closed
priority: low
refs:
- .cue/feat-curator-activity-item/spec/index.md
- .cue/feat-curator-activity-item/plan/1783746571-cfa450e/slice6c-two-pane-activity.md
---
# Collect user turn messages (human prompts)

The activity feed currently only shows agent-side events (turns, tool calls,
session metadata). The human prompts that initiate each turn are invisible.

## Proposed addition

Add a new event type to the pipeline for user messages:
- **Plugin**: capture the user message text from the opencode event stream
  (likely `message.updated` with `role: "user"`) and emit a new `UserMessage`
  event type.
- **Acuity schema**: add `UserMessage { session_id, turn_id, project_dir,
  harness, content: String }` variant.
- **Curator**: render in the Events pane above the `agent_turn_completed` that
  follows, so each turn shows: `[user prompt] -> [tool calls] -> [turn summary]`.

## Trigger

Action when: the flat reverse-chrono event list becomes hard to read without
the human side of the conversation visible, or when implementing stage-C turn
expand/collapse (a natural time to add prompt context to expanded turns).
