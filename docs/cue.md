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

Run `cue --help` for the full command reference.

> This page is a stub. Detailed usage and the artifact format will be
> documented here.
