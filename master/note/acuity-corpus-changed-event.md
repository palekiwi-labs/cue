---
status: open
title: corpus_changed event via tool call observation
---

`acuity` currently collects agent lifecycle events (tool calls, session state).
Tool calls that target `cue-add`, `cue-plan`, `cue-task`, or `Edit` on a path
under `.cue/` are de facto corpus mutation events.

## The idea

A future acuity plugin could derive a `corpus_changed` event by inspecting the
`tool_name` and parameters of incoming `ToolCallCompleted` events. When the tool
is `cue-add`, the event payload already contains the artifact type and path.

This derived event could then serve as a trigger for automated `acumen sync` --
keeping the graph index current without requiring manual invocation or a
filesystem watcher.

## What is needed

- A new `CorpusChanged` event type in `acuity-schema`
- A plugin rule that derives this event from relevant tool calls
- An `acumen` listener that subscribes via the acuity SSE stream, or a webhook
  that triggers `acumen sync`

## Current state

Not implemented. `acuity` is a telemetry-only tool today with no corpus
awareness. This is a valid evolution target once `acumen` has a working sync
pipeline.

## Relationship to `acumen`

See `spec/acumen/index.md` for the full `acumen` design.
