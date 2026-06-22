---
title: Feature Design Questions
priority: normal
status: open
---

## Archiving

A frontmatter field or move archived tasks to `.cue/master/tasks/archived/` subdir?
One pro of a moving to the subdir would be performance of scanning the filesystem:
- all active items in the root
- all archived in `archived/`

## Re-evaluate nesting artifacts in `<timestamp>-<hash>` subdir

This idea predates support of the frontmatter feature in artifacts so
it may be a good moment to take another look at it. Its original purpose
was:
- to record extra information that can also be immediately used for such
  operations as sorting (simply by paths creates a quasi-historical order).
- to allow frictionless saving of artifacts with the same name, e.g. `review.md`
  that will not conflict with each other because they are in different dirs
- frontmatter is only availble on markdown files but nested dirs work for anything
