# cue: Agent Memory System

## Overview
`cue` is a CLI tool and protocol for managing **Agent Memory** and cross-session continuity. It allows AI agents to maintain a persistent state of context, intent, and historical discoveries within a Git-tracked project.

## Goals
- Provide a structured way for agents to record what they are doing (Intent) and what they have found (History).
- Isolate memory artifacts from source code using orphan branches and Git worktrees.
- Ensure branch-specific context isolation.
- Support diverse artifact types including specifications, logs, documentation, and binary data (like clipboard images).

## Core Protocol
- **Spec**: Stable context (goals, plans, todos).
- **Log**: Cumulative history of milestones and decisions.
- **Isolation**: Stored in `.cue/` (mounted via worktree to an orphan branch).
