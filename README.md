# cue

`cue` is a file-based memory system for agentic workflows. It provides a CLI
(`cue`) and library (`cuelib`) that manage structured, branch-isolated
**artifacts** — specs, plans, todos, tasks, traces, and logs — so an agent can
retain its intent, plans, and historical discoveries across sessions. The goal
is to eliminate context drift and redundant research.

This repository is a Cargo workspace containing the `cue` memory core, a live
observability stack (`acuity`), and a terminal kanban view (`curator`).

## Architecture

```
                          cue ecosystem

  FILE-BASED MEMORY   (the mature core)
  -----------------

                    agent
                      |  read / write
                      v
                +--------------+
                |   cue CLI    |   (+ cuelib library)
                +--------------+
                      |
                      | persist
                      v
                +--------------+
                |    .cue/     |   artifacts:
                |   (store)    |   spec / plan / todo /
                +--------------+   task / trace / log
                      |
                      | read
                      v
                +--------------+
                |   curator    |   kanban TUI
                +--------------+

  LIVE OBSERVABILITY
  ------------------

                  agent session
                       |
                       | lifecycle events
                       v
                +--------------+
                | opencode plug|   acuity-schema
                |  (emitter)   |   --ts-rs--> types.ts
                +--------------+
                       |
                       | HTTP POST
                       v
                +--------------+
                |    acuity     |
                | (ingest srv) | ---> SQLite (events.db)
                +--------------+
                       |
                       | optional
                       v
                   +--------+
                   | Gotify |
                   +--------+

  PLANNED (Phase 6):  acuity  --SSE + historical-->  curator (live view)
```

The memory core (`cue`/`cuelib`) and the observability stack (`acuity`) are
independent today; Phase 6 will wire live `acuity` data into `curator`.

The artifact store lives at `<main-git-root>/.cue/`. Linked Git worktrees share
that store, but each worktree keeps its own local `.cue/HEAD` selection. Scoped
commands resolve their context in this order: an explicit `--task`, then
`$CUE_TASK`, then the local `.cue/HEAD`, and finally `master`.

## Install (Nix)

A Nix flake is provided. `cue` is the default package; `curator`, `acuity`,
and `git-pr-sync` (plus `git-scripts`) are available as additional flake outputs.

Run without installing:

```
nix run github:palekiwi-labs/cue             # cue (default)
nix run github:palekiwi-labs/cue#curator
nix run github:palekiwi-labs/cue#acuity
nix run github:palekiwi-labs/cue#git-pr-sync
```

Install to your user profile:

```
nix profile add github:palekiwi-labs/cue             # cue (default)
nix profile add github:palekiwi-labs/cue#curator
nix profile add github:palekiwi-labs/cue#acuity
nix profile add github:palekiwi-labs/cue#git-scripts
```

Or consume the flake from a system configuration — `acuity` ships a
`nixosModules.acuity` output for the managed service (see
[docs/acuity.md](docs/acuity.md)).

A dev shell is available via `nix develop` (or `direnv allow`).

## Git PR Metadata Protocol

Agent harnesses and tooling frequently require branch target and PR metadata
(e.g., base branch, PR number, upstream status) for diff computation, prompt
context injection, and review generation without making network requests on
the hot path.

This repository defines an open storage contract in local repository Git config:

- `branch.<branch>.base`: Target base branch (e.g. `master`, `main`).
- `branch.<branch>.pr`: PR number (e.g. `123`).
- `branch.<branch>.ahead`: Set to `"true"` if upstream base has commits not
  merged into current HEAD; unset otherwise.

Because configuration is stored in `.git/config`, metadata is natively shared
across all Git worktrees and retained across branch checkouts.

### Reference Scripts (`scripts/`)

Portable reference scripts are provided under `scripts/`:

- `git-pr-sync`: Writer script that syncs GitHub PR metadata via `gh` CLI.
  Designed for `post-checkout` / `post-merge` hooks; preserves cached state on
  network or authentication failures, and safely exits with code 0.
- `get-pr-base`: Pure offline reader (<5ms) resolving base branch via Git
  config (`branch.<branch>.base`), `origin/HEAD`, and local ref heuristics.
- `get-pr-number`: Pure offline reader (<5ms) returning `branch.<branch>.pr`.

Custom forge integrators (GitLab, Gitea, Bitbucket) can provide their own sync
scripts populating the standard `branch.<name>.*` keys.

## Docs

- [cue](docs/cue.md) — the memory CLI
- [curator](docs/curator.md) — the artifact kanban TUI
- [acuity](docs/acuity.md) — the observability ingestion server (+ NixOS module)
