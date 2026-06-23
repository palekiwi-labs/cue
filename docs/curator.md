# curator

`curator` is the terminal kanban board for the cue memory system. It reads a
project's `.cue/` tasks and renders Open / In Progress / Complete columns.

It is read-only and runs against the current working directory's project.

## Install

```
nix run github:palekiwi-labs/cue#curator
nix profile add github:palekiwi-labs/cue#curator
```

## Usage

Run from inside a project that has a `.cue/` directory:

```
curator
```

Navigation uses HJKL / arrow keys, with per-column scrolling.

> This page is a stub. Phase 6 will wire live `acuity` activity into the board.
