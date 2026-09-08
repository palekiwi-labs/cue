# cue

`cue` is the file-based memory CLI at the core of the ecosystem. It manages
structured, branch-isolated artifacts (specs, plans, todos, tasks, traces,
logs) under a project's `.cue/` directory so an agent retains intent and
history across sessions.

## Install

```
nix run github:palekiwi-labs/cue
nix profile add github:palekiwi-labs/cue
```

## Usage

Initialize artifact storage in the current project, then add artifacts:

```
cue init
cue add <filename> "<content>"
```

The store is created at `<main-git-root>/.cue/` and is shared by all linked Git
worktrees. Each worktree retains its own local `.cue/HEAD` selection.

Scoped commands resolve their task context in this order:

1. an explicit `--task <slug>` argument;
2. the `$CUE_TASK` environment variable;
3. the worktree-local `.cue/HEAD` file;
4. the global `master` context.

Agents should set `$CUE_TASK` for child processes and sessions so they inherit
the intended task scope without changing the human-owned `.cue/HEAD` file.

Run `cue --help` for the full command reference.

> This page is a stub. Detailed usage and the artifact format will be
> documented here.
