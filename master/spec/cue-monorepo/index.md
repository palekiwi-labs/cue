# Cue Monorepo

## Overview

`cue` is a framework for context and memory sharing between human developers
and AI agents. This repository hosts the full Rust ecosystem as a single Cargo
workspace. TypeScript components live in a separate repository (`cue-plugins`).

## Repository: `palekiwi-labs/cue`

A pure Rust + Nix workspace. All components share the same toolchain and Nix
flake.

### Workspace Members

| Crate | Role |
| --- | --- |
| `cue` | CLI binary for creating and managing cue artifacts |
| `cuelib` | Shared domain library: artifact types, config, git, project management |
| `curator` | TUI dashboard: kanban view of cue artifacts + acuity event data |
| `acuity` | Observability server: collects agent lifecycle events, serves via SSE and query API |
| `acuity-schema` | Ingest wire types only (serde + ts-rs). Shared with the TS plugins. |
| `acuity-api` | Read/response types for acuity's SSE and query API. Used by curator. |

### Crate Dependency Graph

```
cue            -> cuelib
curator        -> cuelib, acuity-api
acuity         -> acuity-schema, acuity-api
acuity-schema  (serde only -- no internal deps)
acuity-api     (serde only -- no internal deps)
```

`acuity` does not depend on `cuelib`. It is a standalone network service with
no knowledge of the `.cue/` filesystem layout.

## Repository: `palekiwi-labs/cue-plugins`

TypeScript packages that plug into agent harnesses (opencode, pi, etc.) and
emit lifecycle events to `acuity`.

### TypeScript <-> Rust Schema Contract

- `acuity-schema` defines ingest payload structs as Rust types with `serde` +
  `ts-rs` derives. It is the single source of truth for the wire format.
- `ts-rs` generates a `types.ts` from those structs, committed into
  `cue-plugins` as a vendored artifact.
- Every POST includes a `X-Acuity-Schema: N` version header so `acuity` can
  detect and reject mismatched schema versions.
- Distribution via Nix flakes is sufficient for current needs. npm publishing
  is deferred.

### Lifecycle Hooks Emitted by Plugins

- Agent session idle
- Agent tool call requested
- Agent tool call completed

## Key Design Decisions

- `acuity-schema` is intentionally thin: ingest wire types only. No error
  types, protocol constants, or storage types belong here.
- `curator` depends on `acuity-api` (read/response types), not `acuity-schema`
  (ingest types). These are separate bounded contexts.
- `cuelib` is shared by `cue` and `curator` only. It is the
  artifact/filesystem domain. `acuity` does not depend on it.
- OpenAPI/AsyncAPI: not adopted. Both wire ends are controlled. Revisit only
  if external consumers emerge.
- `ts-rs` chosen over `typeshare`. Multi-language codegen is deferred until a
  concrete non-TypeScript consumer exists.
- SQLite is the storage backend for `acuity`.

## Deferred

- Auth/trust boundary for `acuity` POST endpoints.
- npm package publishing strategy for the generated `types.ts`.
- Detailed `acuity` API design (endpoints, pagination, query language).
- Detailed `curator` UI design.

