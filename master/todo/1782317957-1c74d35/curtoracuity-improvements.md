---
priority: normal
status: open
title: Curtor/Acuity improvements
---

Some improvements that we should consider for `acuity`, `cue-plugin` and `curator`.

## Send more information from the plugin

The plugin knows the following information that is valuable for our diagnostics
that we cannot or do not need to obtain from the harness itself:

- agent harness name, e.g. `opencode`, `pi`
- current working directory
- hostname, although they run in the container so we may need to think about it

Other than that, we should consider also sending:
- the model being used by the agent (agent turns, tool calls)

Plugins have access to their container bash shell so can collect extra information for us.

If we change the schema or the payload, we do not need to worry about any backwards
compatibility - we can simply start with a fresh database. We are still prototyping.

## `curator`: use progress indicators

Currently, starting `curator` results in a rather jarring experience in the activity
feed and diagnostics views where the servers is streaming pages of results and `curator`
updates them by prepending the newest rows at the top which looks as if the view was
scrolling to the top of the page. If we are loading data, let's use a "loader"/"progress"
widget until we are ready to present the result to the user.

## `curator`: add logging and tracing

Things could go wrong, we need a log for auditing and debugging.

##  `curator`: add a config file

Follow the convention of other `cue` tools, e.g. `~/.config/curator/curator.json`
NOTE: We should consider whether we want to merge the directories into one
directory in `~/.config/cue/` with `cue.json`, `acuity.json`, `curator.json`
all sharing it.
