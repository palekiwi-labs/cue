# Agent Context: mem

This repository is the home of the `mem` CLI tool and protocol—a system designed to manage **Agent Memory** and ensure cross-session continuity.

## Purpose
The `mem` tool solves the problem of "context drift" and redundant research by providing a structured, branch-isolated storage for an agent's intent, plans, and historical discoveries.

## Development Protocol
This project strictly follows the `mem` protocol for its own development:
1. **Load Skills**: Always load the `mem` skill and the `tdd` skill (for Rust development).
2. **Consult the Anchor**: Read `.mem/master/spec/index.md` for the current project state and general specification.
3. **Log Progress**: Use `mem log add` to record milestones and decisions.
