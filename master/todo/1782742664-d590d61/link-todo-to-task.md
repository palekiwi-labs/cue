---
status: open
priority: normal
title: Link todo to task
---
Analyze if we should perhaps tighten the definition and role of `todo`
artifacts, such that on a branch a `todo` is a local extension
of the `task` that we are working on in such a way that a feature branch
should not be merged until all the `todo` artifacts are handled.

Imagine the following:

1. we have a task on master
2. we start a feature branch
3. we have plans on the feature branch
4. some issue is discovered, we capture the error message in a trace artifact

How do we record the fact that we want to solve this newly detected issue?

Would adding a todo artifact with a description and a link to the trace
artifact be the most natural course of action?

Then as part of addressing the todo, we could create another plan artifact.

Creating another `task` on master to address this bug seems a bit heavy.
