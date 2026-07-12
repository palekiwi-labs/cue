---
title: Collect user turn messages (human prompts) in activity feed
status: open
priority: low
refs:
- .cue/feat-curator-activity-item/todo/1783746571-cfa450e/collect-user-turn-messages.md
- .cue/master/spec/index.md
---
# Collect user turn messages (human prompts) in activity feed

The activity feed currently only shows agent-side events (turns, tool calls,
session metadata). The human prompts that initiate each turn are invisible.
Add a new event type for user messages so each turn shows the conversation
context: `[user prompt] -> [tool calls] -> [turn summary]`.

## Source

- Original todo: `.cue/feat-curator-activity-item/todo/1783746571-cfa450e/collect-user-turn-messages.md`
- Spec: `.cue/master/spec/index.md`

## Trigger

Action when:
- The flat reverse-chrono event list becomes hard to read without the human
  side of the conversation visible, OR
- Implementing stage-C turn expand/collapse (a natural time to add prompt
  context to expanded turns).

## Proposed shape

- **Plugin**: capture user message text from the opencode event stream
  (likely `message.updated` with `role: "user"`) and emit a new `UserMessage`
  event.
- **Acuity schema**: add `UserMessage { session_id, turn_id, project_dir,
  harness, content: String }` variant.
- **Curator**: render in the Events pane above the `agent_turn_completed`
  that follows.

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---------------------|-----------|----------|
| 1 | `UserMessage` schema variant exists with round-trip test | unit test | |
| 2 | Plugin handler emits `UserMessage` on user messages | typecheck + live QA | |
| 3 | Curator renders user prompt above its turn in the Events pane | manual QA | |
| 4 | `cargo test --workspace` green | test run | |
| 5 | `cargo clippy --workspace -- -D warnings` clean | clippy run | |
| 6 | `bun run typecheck` (cue-plugins) clean | typecheck | |
