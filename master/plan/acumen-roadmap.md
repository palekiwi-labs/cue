---
status: open
refs:
  - "master/spec/acumen/index.md"
---
# Acumen Implementation Roadmap

Produced from a design discussion session. The graph model, storage backend,
and frontmatter additions were finalized before any implementation began.

Supersedes: `note/acumen-with-graph-db.md`, `note/acumen/cue-context-with-graph-db.md`

---

## Phase 0 -- Frontmatter foundations

**What**: Add `refs:` and `git_branch:` as first-class parsed fields in
`cuelib`'s `RawFrontmatter`. Extend the cue skill in `cue-plugins` to mandate
`refs:` on all artifacts. Create the branch-init shell script (in the NixOS
flake) that writes `git_branch:` to `spec/index.md` from
`git rev-parse --abbrev-ref HEAD`.

**Why first**: The graph is only as rich as the edges it can extract. Until
`refs:` is parsed and mandated, the mapper has no edges to create. This phase
delivers independent value to `cue list` and `curator` regardless of `acumen`.

**Done**: `RawFrontmatter` parses `refs` and `git_branch`. The cue skill
documents `refs:` as mandatory. The branch-init script creates `spec/index.md`
with `git_branch:` set correctly.

---

## Phase 1 -- `acumen` crate scaffold + SQLite schema

**What**: Add the `acumen` crate to the workspace. Define the node/edge Rust
types in `schema.rs`. Define the `GraphBackend` trait in `backend.rs`. Implement
the SQLite backend with the schema from the spec.

**Why before the mapper**: Establish the data model in types and schema before
writing code that populates it. Validate with in-memory SQLite for fast
iteration in tests.

**Done**: `cargo build` succeeds across the workspace. The SQLite schema creates
without error. An in-memory test inserts a node and an edge and reads them back.

---

## Phase 2 -- Filesystem parser + `acumen sync`

**What**: Implement the `.cue/` walker in `sync.rs`: walk branch directories,
parse frontmatter, extract `refs:`, compute `content_hash`, resolve paths,
upsert nodes, delete removed nodes, rebuild edges. Implement
`parser/frontmatter.rs` and `parser/refs.rs`.

**Why this is the riskiest phase**: Edge extraction quality depends on
frontmatter discipline. The mapper must handle non-markdown files (nullable
fields), missing `refs:`, and malformed frontmatter gracefully without
panicking.

**Done**: `acumen sync` runs against a real `.cue/` directory without error.
`acumen sync && acumen sync` produces identical graph state (idempotency
invariant). Node count matches the number of files under `.cue/`.

---

## Phase 3 -- Named queries + CLI surface

**What**: Implement the four named queries (`related`, `lineage`, `stale`,
`orphans`) in `queries.rs`. Wire them to the `acumen query` CLI subcommand.
Implement `acumen status`.

**Done**: All four queries return correct results against a real synced graph.
`acumen status` reports node count, edge count, last sync time, DB path.

---

## Phase 4 -- `cue query` integration

**What**: Import `acumen` as a library dependency in the `cue` crate. Add
`cue query` as a subcommand that delegates to acumen's named queries.

**Done**: `cue query related --to master/spec/acumen/index.md` returns correct
results. `cue` degrades gracefully when the graph DB does not exist, printing
an actionable message: "run acumen sync first".

---

## Phase 5 -- Handoff skill + post-commit hook

**What**: Write the handoff skill in `cue-plugins`. Write the post-commit hook
shell script in the NixOS flake. The hook prints a reminder to load the handoff
skill when ending a session.

**Done**: Post-commit hook prints the handoff reminder to stdout. The handoff
skill checklist is complete and covers: `spec/index.md` exists, `git_branch:`
is set, all artifacts created in the session have `refs:` populated.

---

## Deferred phases

- Neo4j backend (add behind the `GraphBackend` trait when needed)
- `acuity`-driven sync via `CorpusChanged` event
- Schema versioning
- Semantic search over `doc/` artifacts
- `curator` integration (graph health signals on the kanban board)
