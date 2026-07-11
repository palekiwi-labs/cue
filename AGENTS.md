# Agent Context: cue

<important>always load the "cue" skill first</important>

<important>
we are at a prototyping stage:
- do not bump package or schema versions
- do not worry about backwards compatibility
</important>

This repository is the home of the `cue` CLI tool and protocol—a system designed to manage **Agent Memory** and ensure cross-session continuity.

## Purpose
The `cue` tool solves the problem of "context drift" and redundant research by providing a structured, branch-isolated storage for an agent's intent, plans, and historical discoveries.

## Development Protocol
This project strictly follows the `cue` protocol for its own development:
1. **Load Skills**: Always load the `cue` skill and the `tdd` skill (for Rust development).
2. **Consult the Anchor**: Read `.cue/master/spec/index.md` for the current project state and general specification.
3. **Log Progress**: Use `cue log add` to record milestones and decisions.

## External directories

- `/home/pl/code/palekiwi-labs/cue.nvim`: a neovim plugin for cue
- `/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins`: cue plugins for agent harnesses

<important>
In order to access depencencies of projects in external directories,
use `nix develop` from within those directories to mak use of devshells
defined in their `flake.nix`
</important>
