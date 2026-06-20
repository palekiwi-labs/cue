---
status: open
---
# Cue Ecosystem Implementation Roadmap

Produced via three rounds of Opus consultation during a design/discussion
session. Revised once to pull the `acuity` stateless MVP before `curator`
after identifying it as a better proof of the cross-repo schema contract.

Supersedes: `trace/1781942441-cef325f/cue-ecosystem-roadmap.md`

## Ordering Principles

1. Protect the asset. `cue`/`cuelib` is the mature, working component. Anything
   that adds a new consumer must not regress it.
2. De-risk unknowns early, but cheaply. The riskiest new thing is the
   cross-repo, cross-language schema contract (`acuity-schema` -> `ts-rs` ->
   `types.ts`). Prove that pipeline with a real consumer before building
   anything heavier on top of it.
3. Always have a runnable, demoable artifact. Each phase ends with something
   you can actually run.

---

## Phase 0 — Workspace & contract scaffolding

**What:** Add the four new crate skeletons (`acuity-schema`, `acuity-api`,
`acuity`, `curator`) to the workspace, wired into the dependency graph but
empty. Wire up `ts-rs` in `acuity-schema` and establish the codegen command
and the `cue-plugins` repo to receive the vendored output.

**Why first:** Locks in the dependency graph and the tooling before any code
makes either hard to change. The codegen command must exist before Phase 1 can
define a real event type and generate from it.

**Task:** `task/1781942441-cef325f/workspace-scaffold.md`

**Done:** `cargo build` succeeds across all six crates. The codegen command
exists and is repeatable. `cue-plugins` repo is initialised and ready to
receive a vendored `types.ts`.

---

## Phase 1 — `acuity` stateless MVP: `session.idle` -> Gotify

**What:** Define the `SessionIdle` event type in `acuity-schema` with `serde`
+ `ts-rs` derives. Generate `types.ts` and commit it into `cue-plugins`. Build
the `acuity` binary with a single POST endpoint that accepts a `session.idle`
event, validates the `X-Acuity-Schema` header, and forwards a notification to
a configured Gotify instance. Write the opencode plugin in `cue-plugins` using
the vendored type.

**No SQLite. No second event type. No SSE or query surface.**

**Why before `curator`:** A codegen pipeline with no live consumer is compiled
but not proven. The existing hand-rolled notifications server provides a real
consumer with a known payload and a downstream (Gotify) already in production.
This phase retires the #1 architectural risk — the cross-repo schema contract —
in the cheapest possible way, and simultaneously replaces a live production
dependency. `curator` slips by one slot; the risk reduction is not time-
sensitive.

**Task:** `task/1781965432-d2f3251/acuity-stateless-mvp.md`

**Done:** A real `session.idle` POST from the opencode plugin, carrying a type
imported from the vendored `types.ts` with a correct `X-Acuity-Schema` header,
is deserialized by `acuity` and forwarded to Gotify. A deliberately wrong
schema version is cleanly rejected. The hand-rolled notifications server is
decommissioned.

---

## Phase 2 — `curator`, artifact half (read-only kanban over `.cue/`)

**What:** Build `curator` reading from `cuelib` and rendering a kanban board
of tasks/plans/todos across all registered projects. No `acuity` involvement.

**Why here:** Now unblocked and the next highest-value, lowest-risk item.
Exercises `cuelib` under a second consumer, surfacing any API assumptions
shaped only for the CLI.

**Task:** `task/1781965432-d2f3251/curator-artifact-kanban.md`

**Done:** `curator` launches in a real project and shows the actual board.
`cue` CLI tests still pass.

---

## Phase 3 — `acuity-schema` full event model + `acuity` ingest + SQLite

**What:** Extend `acuity-schema` with the remaining lifecycle event types
(tool-call-requested, tool-call-completed) and the ingest envelope (`seq`,
`received_at`). Add SQLite persistence to `acuity`'s POST handler.

**Why after Phase 1:** The schema contract is already proven; this phase
widens it. SQLite is introduced only once the ingest path is known-good from
Phase 1. Validate with `curl` before introducing additional harness variables.

**Task:** `task/1781965432-d2f3251/acuity-full-event-model.md`

**Done:** `curl` POSTs all three event types, rows land in SQLite with correct
envelope fields, wrong schema version is cleanly rejected.

---

## Phase 4 — `cue-plugins`: full emitter (one harness only)

**What:** Extend the opencode plugin (or add pi) to also emit tool-call events
using the updated `types.ts` from Phase 3.

**Why only one harness:** The second adds breadth, not new knowledge. Prove
the full contract once; generalise later.

**Task:** `task/1781965432-d2f3251/cue-plugins-first-emitter.md`

**Done:** A live agent session produces real events of all three types in
`acuity`'s SQLite. A deliberate schema bump produces a clean rejection at the
plugin boundary.

---

## Phase 5 — `acuity-api` read model: query API + SSE

**What:** Define `acuity-api`'s read/response types. Implement `acuity`'s
outbound surface: historical query API and real-time SSE stream.

**Why after a full emitter:** Real data is now in SQLite. Designing query
shapes against actual traffic produces better types than guessing at fixtures.

**Task:** `task/1781965432-d2f3251/acuity-read-model.md`

**Done:** HTTP queries return sensible results. SSE stream delivers live events
as an agent runs. Both validated independently of `curator`.

---

## Phase 6 — `curator` live half: wire in `acuity`

**What:** Add `acuity-api` as a `curator` dependency. Integrate the live view:
SSE-driven events and historical aggregates (token counts per task, session
idle state) overlaid on the Phase 2 kanban.

**Why last among the core phases:** Convergence point — every input is
independently proven. Failures here can only be in the seam, not the
components.

**Task:** `task/1781965432-d2f3251/curator-live-half.md`

**Done:** With a live agent running and `acuity` up, `curator` shows the board
and live activity updating in real time. Full ecosystem loop closed:
agent -> plugin -> acuity -> curator.

---

## Phase 7 — Hardening & deferred concerns

**What:** Auth/trust boundary for `acuity` POSTs (mandatory before any
multi-host deployment), SQLite retention/pruning, second harness in
`cue-plugins`, multi-host connection config for `curator <-> acuity`.

**Why deferred:** None block proving the architecture. Auth is sequenced ahead
of any cross-machine deployment because an unauthenticated ingest endpoint on
a shared network is the one deferred item that is a real liability.

**Task:** `task/1781965432-d2f3251/acuity-hardening.md`

**Done:** `acuity` runs safely on a separate host with authenticated ingest.
`curator` connects to it via config.

---

## Sequence at a Glance

| Phase | Deliverable                                    | Risk Retired                                                     |
| ----- | ---------------------------------------------- | ---------------------------------------------------------------- |
| 0     | Workspace + codegen wiring                     | Crates compile; codegen command exists                           |
| 1     | `acuity` stateless MVP: session.idle -> Gotify | Cross-repo schema contract proven end-to-end; replaces prod server |
| 2     | `curator` artifact kanban                      | `cuelib` as 2nd consumer; daily-driver value                     |
| 3     | `acuity-schema` full events + SQLite ingest    | Wire format + persistence                                        |
| 4     | `cue-plugins` full emitter (one harness)       | Real end-to-end emission                                         |
| 5     | `acuity-api` query + SSE                       | Read model against real data                                     |
| 6     | `curator` live half                            | Full-loop integration                                            |
| 7     | Auth, retention, 2nd harness, multi-host       | Deferred production concerns                                     |

## Key Ordering Decisions

- **Phase 1 (acuity MVP) before Phase 2 (curator):** A codegen pipeline with
  no live consumer is unproven. The stateless MVP retires the #1 architectural
  risk against a real production need. Curator slips one slot; the tradeoff is
  unambiguous.
- **Phase 1 is stateless by hard constraint:** No SQLite, no second event type,
  no query/SSE surface. The moment persistence is needed, it is a new phase
  (Phase 3). This boundary prevents Phase 1 from silently becoming Phase 3.
- **Phase 3 before Phase 4:** Build the full persistent sink you control, prove
  it with `curl`, then point a real harness at it. Plugin debugging must never
  be tangled with server debugging.
