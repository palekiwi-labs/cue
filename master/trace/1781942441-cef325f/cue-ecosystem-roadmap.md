# Cue Ecosystem Implementation Roadmap

Produced via two rounds of Opus consultation during a design/discussion session.
Not a plan — a sequenced set of phases with ordering rationale for use as a
prioritisation reference when creating kanban tasks.

## Ordering Principles

1. Protect the asset. `cue`/`cuelib` is the mature, working component. Anything
   that adds a new consumer must not regress it.
2. De-risk unknowns early, but cheaply. The riskiest new thing is the
   cross-repo, cross-language schema contract (`acuity-schema` -> `ts-rs` ->
   `types.ts`). Prove that pipeline before building anything that rides on it.
3. Always have a runnable, demoable artifact. Each phase ends with something
   you can actually run.

---

## Phase 0 — Workspace & contract scaffolding

**What:** Add the four new crate skeletons (`acuity-schema`, `acuity-api`,
`acuity`, `curator`) to the workspace, wired into the dependency graph but
empty. Prove the `acuity-schema` -> `ts-rs` -> `types.ts` codegen pipeline
end-to-end with one trivial type. Establish the vendoring workflow in
`cue-plugins`.

**Why first:** The cross-language, cross-repo schema contract is the biggest
architectural unknown. Validate it with a one-field struct before designing a
full event schema. If the codegen is awkward in a Nix environment, discover
it now.

**Done:** `cargo build` succeeds across all six crates. A Rust struct produces
a `types.ts` committed into `cue-plugins`. A repeatable codegen command exists.

---

## Phase 1 — `curator`, artifact half (read-only kanban over `.cue/`)

**What:** Build `curator` reading from `cuelib` and rendering a kanban board
of tasks/plans/todos across all registered projects. No `acuity` involvement.

**Why before `acuity`:** Unblocked right now. Pure payoff (daily-driver tool)
with zero new infrastructure. Also exercises `cuelib` under a second consumer,
surfacing any API assumptions shaped only for the CLI.

**Done:** `curator` launches in a real project and shows the actual board.
`cue` CLI tests still pass.

---

## Phase 2 — `acuity-schema` real event model + `acuity` ingest + storage

**What:** Flesh out the three lifecycle event types in `acuity-schema` with
the ingest envelope (`seq`, `received_at`). Build `acuity`'s POST endpoint(s):
accept events, validate `X-Acuity-Schema` header, reject mismatches, persist
to SQLite.

**Why before the plugins:** Settle event shapes while both ends are controlled.
Validate with `curl` before introducing a TypeScript harness as a variable.
Schema-version rejection logic is cheaper to get right before a second language
depends on it.

**Done:** `curl` POSTs all three event types, rows land in SQLite with correct
envelope fields, wrong schema version is cleanly rejected.

---

## Phase 3 — `cue-plugins`: first real emitter (one harness only)

**What:** TypeScript plugin for one harness (opencode or pi — not both) hooking
the three lifecycle events and POSTing them using the vendored `types.ts`.

**Why only one harness:** The second adds breadth, not new knowledge. Prove the
contract once. First real exercise of the cross-repo workflow from Phase 0 with
a non-trivial schema.

**Done:** A live agent session produces real idle/tool-call events in `acuity`'s
SQLite. A deliberate schema bump produces a clean rejection at the plugin
boundary.

---

## Phase 4 — `acuity-api` read model: query API + SSE

**What:** Define `acuity-api`'s read/response types. Implement `acuity`'s
outbound surface: historical query API and real-time SSE stream.

**Why after a real emitter:** Real data is now in SQLite. Designing query shapes
against actual traffic produces better types than guessing at fixtures.

**Done:** HTTP queries return sensible results. SSE stream delivers live events
as an agent runs. Both validated independently of `curator`.

---

## Phase 5 — `curator` live half: wire in `acuity`

**What:** Add `acuity-api` as a `curator` dependency. Integrate the live view:
SSE-driven events and historical aggregates (token counts per task, session idle
state) overlaid on the Phase 1 kanban.

**Why last among the core phases:** Convergence point — every input is
independently proven. Failures here can only be in the seam, not the components.
This is where the hardest wiring belongs (async SSE in a TUI event loop,
reconciling two live data sources).

**Done:** With a live agent running and `acuity` up, `curator` shows the board
and live activity updating in real time. Full ecosystem loop closed:
agent -> plugin -> acuity -> curator.

---

## Phase 6 — Hardening & deferred concerns

**What:** Auth/trust boundary for `acuity` POSTs (mandatory before any
multi-host deployment), SQLite retention/pruning, second harness in
`cue-plugins`, multi-host connection config for `curator <-> acuity`.

**Why deferred:** None block proving the architecture. Auth is sequenced ahead
of any cross-machine deployment because an unauthenticated ingest endpoint on
a shared network is the one deferred item that is a real liability.

**Done:** `acuity` runs safely on a separate host with authenticated ingest.
`curator` connects to it via config.

---

## Sequence at a Glance

| Phase | Deliverable                          | Risk Retired                          |
| ----- | ------------------------------------ | ------------------------------------- |
| 0     | Workspace + codegen loop             | Cross-language schema contract        |
| 1     | `curator` artifact kanban            | `cuelib` as 2nd consumer; early value |
| 2     | `acuity-schema` events + ingest      | Wire format + persistence             |
| 3     | `cue-plugins` (one harness)          | Real end-to-end emission              |
| 4     | `acuity-api` query + SSE             | Read model against real data          |
| 5     | `curator` live half                  | Full-loop integration                 |
| 6     | Auth, retention, 2nd harness         | Deferred production concerns          |

## Key Ordering Decisions

- **Phase 1 before Phase 2**: Value-first. `curator`'s artifact half is
  unblocked today and delivers a daily-driver tool while the backend is still
  being built.
- **Phase 2 before Phase 3**: Build the sink you fully control and validate
  it with `curl` before pointing a real agent harness at it. Plugin debugging
  must never be tangled with server debugging.
