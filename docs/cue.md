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

The central store root is selected by `$CUE_STORE` or defaults to `~/cue`.
Repositories are partitioned by their origin-derived `<org>/<repo>` scope, so
linked worktrees and clones of the same origin share context without project-
local cue files.

Scoped commands resolve their active context in this order:

1. an explicit `--context <slug>` argument;
2. the `$CUE_CONTEXT` environment variable;
3. `branch.<current-branch>.cue-context` in local Git configuration;
4. unset.

Agents should set `$CUE_CONTEXT` for child processes and sessions so they
inherit the intended context without changing repository configuration.

Run `cue --help` for the full command reference.

> This page is a stub. Detailed usage and the artifact format will be
> documented here.
