---
priority: normal
title: Log parsing
status: open
---

The "log" in `cue` and the "cue framework" is a markdown file so
that both humans and agents could read it easily.

However, the log tends to grow in size over time. Can we do anything
to make it parsable and allow users/agents to, e.g. read only
the last 10 entries? How about adding a `---` separator between
doc entries?
