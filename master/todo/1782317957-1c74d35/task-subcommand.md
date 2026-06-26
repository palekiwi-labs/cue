---
title: Task subcommand
status: open
priority: normal
---

Let's think if we need a `task` subcommand that would be specialized
to the management or listing of tasks or is it something that should
not get specialized handling.

We already can create, list and filter artifacts of any type so there
is nothing that a `task` subcommand would meaningfully add here
except a more streamlined interface for agents that would allow them
to quickly orient themselves without constructing complex commands:
- what tasks are there
- display tasks by group (open, in-progress, complete) with extra info like title, maybe a description?
- what is the next task based on some criteria like priority

However one seqence of actions that I find myself repeating manually is
picking up tasks from the stack. i.e:
- setting the `status` to `in-progress`
- adding and setting the `branch` fronmatter field to `<branch-name>`

Perhaps this really should not be part of the `cue` API but rather
users should create scripts (that we could ship as part of `cue-plugins`
together with the `cue` skill) which can bundle these pipelines.

In fact, a `cue-task-start` script could take <task-path> and <branch-name> as arguments
and ensure:
- <branch-name> is created and is checked out
- task status and branch are set
- any extra steps that we want to experiment with

The script approach could allow more flexibility.
