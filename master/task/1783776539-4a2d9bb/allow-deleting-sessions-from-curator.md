---
status: open
title: allow deleting sessions from curator
priority: normal
---
## Context

Sometimes sessions are created by accident and contain no real content,
sometimes we just want to get rid of a useless session data. However, although
we can easily delete sessions inside our agent harnesses, their data has already
been sent to `acuity` and persisted in the DB which then continues serving us
this data. 

## Purpose and Scope

Be able to delete sessions and all event beloging to that session from
`curator`. `curator` should send a delete request to `acuity` and `acuity`
should delete these rows from the DB.
