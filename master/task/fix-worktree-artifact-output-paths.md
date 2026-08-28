---
title: Fix worktree artifact output paths
status: in-progress
priority: high
refs: .cue/master/spec/index.md
kind: build
---
Fix CLI artifact creation output so every printed path can be opened directly from the caller's current working directory, including invocation from linked Git worktrees. Verify the regression through the public CLI before implementing the correction.