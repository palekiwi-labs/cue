---
title: Rename to cue
status: open
priority: 0
---

I have decided to rename `mem` project to `cue`.

The first reason is that I think that `mem` is too generic and boring
a name. The second reason is that I want to split the project into
two crates:

- `cue`: current CLI
- `cueban`: a TUI app (with Ratatui) for management of todos in the project and cross project

The task is to run a complete migration of the source code and the github repo.
