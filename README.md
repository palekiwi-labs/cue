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

The artifact store is central, at `$CUE_STORE` (default `~/cue`), with one
scope per repository derived from its origin remote. Scoped commands resolve
their context in this order: an explicit `--context`, then `$CUE_CONTEXT`,
then the current branch's `branch.<name>.cue-context` Git configuration.
With none of these the context is unset: reads widen to the repository
scope and writes fail.

## Install (Nix)

A Nix flake is provided. `cue` is the default package; `curator` and
`acuity` are available as additional flake outputs.

Run without installing:

```
nix run github:palekiwi-labs/cue             # cue (default)
nix run github:palekiwi-labs/cue#curator
nix run github:palekiwi-labs/cue#acuity
```

Install to your user profile:

```
nix profile add github:palekiwi-labs/cue             # cue (default)
nix profile add github:palekiwi-labs/cue#curator
nix profile add github:palekiwi-labs/cue#acuity
```

Or consume the flake from a system configuration — `acuity` ships a
`nixosModules.acuity` output for the managed service (see
[docs/acuity.md](docs/acuity.md)).

A dev shell is available via `nix develop` (or `direnv allow`).

## Git PR Metadata Scripts

The Git PR metadata scripts (`git-pr-sync`, `get-pr-base`, `get-pr-number`)
and the `branch.<branch>.base` / `.pr` / `.behindBase` / `.behindDefault`
storage contract they implement now live in
[palekiwi-labs/git-pr-sync](https://github.com/palekiwi-labs/git-pr-sync).

## Docs

- [cue](docs/cue.md) — the memory CLI
- [cue-agent](docs/cue-agent.md) — the named-agent delegation runner
- [curator](docs/curator.md) — the artifact kanban TUI
- [acuity](docs/acuity.md) — the observability ingestion server (+ NixOS module)
