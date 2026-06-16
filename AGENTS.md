# Agent Context: cue

This repository is the home of the `cue` CLI tool and protocol—a system designed to manage **Agent Memory** and ensure cross-session continuity.

## Purpose
The `cue` tool solves the problem of "context drift" and redundant research by providing a structured, branch-isolated storage for an agent's intent, plans, and historical discoveries.

## Development Protocol
This project strictly follows the `cue` protocol for its own development:
1. **Load Skills**: Always load the `mem` skill and the `tdd` skill (for Rust development).
2. **Consult the Anchor**: Read `.mem/master/spec/index.md` for the current project state and general specification.
3. **Log Progress**: Use `cue log add` to record milestones and decisions.
