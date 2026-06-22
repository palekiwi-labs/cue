---
title: Ship without a branch_name in config?
priority: normal
status: open
---

We currently ship `cue` with a default `branch_name` config field
set to "cue". Should we consider shipping with `None` and force
the user to set this name for themselves?

## Pros

The benefit would be avoiding a race-condition situation where
multiple users create a personal branch with the same name,
unintentionally pull or push to a colleague's branch, etc.

If `cue` is run without an explicitly set `branch_name`,
then error out with a message.

## Cons

Shipping witout a default branch would hinder the user from
using cue immediately and would force them to manually create
the config directory setup with the correct setting.
Using `cue` as a default branch name is not an unresasonable choice.
