---
title: curator config
priority: normal
status: closed
---

We need a way to configure `curator` globally, most importantly
to set the acuity server url. `cue` supports both environment
variables and global/local config files that is merged by
`figment` crate. `curator` currently does not have many
options to configure but being a TUI there will definitely be
many things that a user may want to configure.

Should we add a configuration solution for `curator` in this phase?
Should we also introduce `curator config show` similarly to `cue`?
