# Acumen Specification

## Purpose

`acumen` is a graph index for the cue ecosystem. It ingests the structured
artifacts stored in `.cue/` directories and persists them as a queryable graph,
enabling agents and tools to traverse relationships between artifacts, navigate
branch lineage, and audit the health of the cue corpus.

`acumen` does not replace `.cue/` as the source of truth. The markdown files
are always authoritative. The graph is a derived, rebuildable index -- a
materialized view of the filesystem.

## Problem

The `.cue/` corpus accumulates artifacts across branches and sessions. As it
grows:

- **Retrieval degrades**: agents must navigate the filesystem manually to find
  relevant context
- **Relationships are implicit**: task -> spec, plan -> task, note -> doc are
  semantically real but not machine-traversable
- **Inter-branch context is lost**: an agent on branch B cannot easily navigate
  the plans, decisions, and task history of a merged ancestor branch A
- **Gardening is hard**: identifying stale todos, orphaned notes, or tasks with
  incomplete evidence requires manual inspection

## Scope

`acumen` solves the traversal and indexing problem. It is explicitly not:

- A semantic search / RAG system (deferred)
- A source of truth for artifact state (`.cue/` markdown files hold that)
- An active gardener that mutates `.cue/` files (passive reporter only)
- A replacement for `acuity` (different data plane: corpus vs session telemetry)

## Graph Model

### Nodes

Every file in `.cue/` is a node. All nodes share a common set of properties;
frontmatter-derived properties are nullable (non-markdown files have no
frontmatter).

| Property | Type | Source | Nullable |
|---|---|---|---|
| `id` | TEXT PK | Full path relative to `.cue/` | No |
| `type` | TEXT | Directory name (`spec`, `task`, ...) | No |
| `branch` | TEXT | Parent dir of type dir | No |
| `git_branch` | TEXT | `git_branch:` frontmatter on `spec/index.md` | Yes |
| `status` | TEXT | Frontmatter | Yes |
| `title` | TEXT | Frontmatter | Yes |
| `priority` | TEXT | Frontmatter | Yes |
| `root` | BOOL | Whether path has no timestamp dir | No |
| `timestamp_dir` | TEXT | Extracted from path | Yes |
| `created_at` | INTEGER | Unix ts from `timestamp_dir` prefix | Yes |
| `updated_at` | INTEGER | `git log -1 --format="%at"` / fs mtime fallback | Yes |
| `content_hash` | TEXT | SHA256 of file content | No |

### Edges

Edges are extracted from `refs:` frontmatter. Every entry in `refs:` creates a
`LINKS_TO` edge. The relationship type can be inferred at query time from the
node types at each end, but is not stored as a named type in the MVP.

```
(source: Artifact) -[:LINKS_TO]-> (target: Artifact)
```

Additional structural edges derived from the directory layout:

| Relationship | Source -> Target | Derived from |
|---|---|---|
| `IN_BRANCH` | Artifact -> Branch | Directory path |
| `PRECEDES` | Artifact -> Artifact | Timestamp ordering, same type + branch |

`PRECEDES` is only created for artifact types where sequence is meaningful:
`plan/` and `trace/`. It is not created for `todo/`, `note/`, or `tmp/`.

`COLOCATED` (same timestamped dir) is intentionally omitted -- it is O(n^2)
per directory with low signal value.

### Branch Nodes

| Property | Type | Source |
|---|---|---|
| `id` | TEXT PK | Directory name under `.cue/` |
| `git_branch` | TEXT | `git_branch:` in `spec/index.md` frontmatter |

The mapping between a flattened `.cue/` directory name and the true git ref is
stored in `spec/index.md`'s `git_branch:` frontmatter field. The directory name
is the storage label; `git_branch:` is the authoritative lookup key.

## Frontmatter Additions to `cue` Artifacts

Two new fields are mandated going forward. Both are enforced by convention (cue
skill protocol) and by the handoff skill at session end.

### `refs:` (any artifact)

A flat list of artifact paths relative to `.cue/`. Written and maintained by
agents following the cue skill protocol.

```yaml
refs:
  - "master/spec/acumen/index.md"
  - "master/task/acumen-build.md"
```

No typed relationship syntax. Relationship semantics are inferred from node
types at query time.

### `git_branch:` (`spec/index.md` only)

The true git ref for the branch. Written once at branch initialization by the
branch-init script. Resolves the slash-flattening ambiguity between `.cue/`
directory names and real git refs.

```yaml
git_branch: "feat/my-feature"
```

## Storage Backend

SQLite. File-backed, embedded, daemon-free. Stored at `.cue/.acumen/graph.db`
and gitignored.

The graph is a derived artifact, not a source of truth. It is fully and
deterministically reconstructable from the git-tracked contents of `.cue/`
alone. Losing the file costs nothing; `acumen sync` rebuilds it in seconds.

The SQLite implementation is behind a `GraphBackend` trait so that a Neo4j
backend can be added later without changing the query layer.

## Sync Strategy

`acumen sync` performs a full rebuild:

1. Walk all `.cue/` branch directories
2. For each artifact: parse frontmatter, extract `refs:`, compute `content_hash`
3. Diff against stored node set: upsert changed nodes, delete removed nodes
4. Rebuild all edges from `refs:` and structural rules

Sync is idempotent: two consecutive syncs with no filesystem changes produce
identical graph state. This is the invariant that justifies gitignoring the DB.

**MVP trigger**: manual (`acumen sync`). Future: post-commit hook on the `.cue/`
worktree, or driven by `acuity` corpus-changed events.

## Query API

Named queries are the primary surface. The agent-facing documentation describes
them; agents call them by name and never write raw SQL.

```
acumen query related  --to <artifact-id>
acumen query lineage  --branch <branch-dir-name>
acumen query stale    --type <type> --older-than <days>
acumen query orphans
acumen status
```

## Use Cases

### 1. Cross-session context retrieval

An agent starting work on a new branch can ask:

- "What specs are related to this task?" (`acumen query related`)
- "What did previous branches working on this feature decide?"
  (`acumen query lineage`)

### 2. Corpus health reporting (passive)

`acumen query stale` and `acumen query orphans` surface health signals. The
agent or human then acts. `acumen` never mutates `.cue/` files directly.

## Architecture

`acumen` is a crate in the cue Cargo workspace.

```
crate: acumen
  src/
    lib.rs             -- public API (imported by cue for cue query)
    graph/
      schema.rs        -- node/edge Rust types
      backend.rs       -- GraphBackend trait
      sqlite.rs        -- SQLite implementation
      sync.rs          -- .cue/ filesystem -> graph mapper
      queries.rs       -- named query implementations
    parser/
      frontmatter.rs   -- YAML extraction (refs:, git_branch:, etc.)
      refs.rs          -- refs: link resolution
    bin/
      main.rs          -- acumen CLI binary
```

`cue` imports `acumen` as a library for the `cue query` subcommand. `acumen`
is also a standalone binary for direct invocation and scripting.

## Configuration

`~/.config/acumen/acumen.json` with `ACUMEN_` environment variable overrides,
following the same layered model as `acuity`. The graph DB path defaults to
`.cue/.acumen/graph.db` relative to the detected `.cue/` root.

## Relationship to the Ecosystem

- **`cuelib`**: `acumen` reads the same `.cue/` filesystem that `cuelib`
  manages. It does not depend on `cuelib` directly -- it reads files
  independently. Future: share the frontmatter parser.
- **`acuity`**: Different data plane. `acuity` observes agent behavior; `acumen`
  indexes the corpus those behaviors produce. Future: `acuity` emits
  `CorpusChanged` events; `acumen` subscribes and triggers incremental sync.
  See `note/acuity-corpus-changed-event.md`.
- **`curator`**: `curator` could import `acumen` as a library to surface graph
  health signals on the kanban board.
- **`cue` CLI**: imports `acumen` as a library for the `cue query` subcommand.

## Deferred

- Semantic search over `doc/` artifacts
- Active gardening (mutations to `.cue/` files)
- Neo4j backend (add behind `GraphBackend` trait when needed)
- `acuity`-driven sync trigger
- Schema versioning (needed before any production deployment)
- Multi-project global graph instance
