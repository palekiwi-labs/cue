# Project Log

## [0902ec0] Build task initialized: branch, context, port kit, seeded plan

Initialized the cue-agent build task. Created branch feat/cue-agent (worktree worktrees/feat-cue-agent, base master) and a self-contained task context: spec snapshot from the upstream design (palekiwi workspace, design-cue-agent), pi 0.84.2 API + validated JSON event schema refs, cast-agent port kit (verbatim supervisor.rs snapshot + distilled invariants), port-assessment trace, and a seeded 5-phase master plan. No code written — implementation is delegated to the build agent working this task.

- **Found:** cue repo .cue store is a separate worktree on branch cue-palekiwi; crates layout is acuity/acuity-api/acuity-schema/cue/cuelib/curator — no cue-agent yet, no slug collision
- **Found:** cue-task tool prepends generated frontmatter; embedded frontmatter in content must be avoided for that tool (fixed manually on the task card)
- **Decided:** Build task context is fully self-contained: no dependency on the palekiwi workspace or the cast-agent worktree surviving
- **Decided:** supervisor.rs transplanted as verbatim starting point; deltas (stdin->@file positional, 5s grace, env overlay, batch-level signals) documented in supervisor-invariants.md
- **Decided:** Prompt delivery reconciled as pi @file positional (prompt.md artifact first, persist-before-spawn) resolving the spec's argv tension
- **Decided:** Old cast-agent exit-code table dropped; spec contract 0/1/2 with crashed folded into failed
- **Open:** cuelib API may lack a helper for writing tmp point-in-time artifact directories (.cue/<ctx>/tmp/<ts>-<name>/) — flagged in plan notes for the implementer to verify early
- **Open:** cue-plugins delegate build task to be created when slice 1 nears completion (tracked in palekiwi coord plan)

## [3d08d58] Commit 3d08d58: cue-agent scaffold + CLI shell (phase 0 start)

First commit of the cue-agent crate. Created crates/cue-agent (package cue-agent, lib cue_agent) wired into the workspace Cargo.toml. The run subcommand has the three spec input modes (positional JSON string, --spec-file PATH, '-' stdin) with clap-level conflict detection between positional and --spec-file. Usage errors exit 2 with an error message on stderr and empty stdout. Successful spec input reads exit 0 silently in this phase; execution lands in phase 1. Phase 0 executive plan cut at .cue/cue-agent/plan/1787477737-0902ec0/phase0-scaffold-parsing.md. Six integration tests via assert_cmd pin the input-mode contract. Deviation from session bootstrap: user instructed to check out feat/cue-agent in the repo root instead of the worktree (worktrees/feat-cue-agent removed).

- **Found:** cuelib exposes pub validate_slug (crates/cuelib/src/head.rs:39) reusable for --task validation
- **Found:** assert_cmd + predicates + tempfile are the established integration-test stack in this repo (crates/cue/tests/helpers.rs)
- **Decided:** phase-0 shell success path: exit 0 with empty stdout; receipts replace it in phase 1
- **Decided:** own errors printed as 'error: <msg>' single line on stderr
- **Decided:** input-mode conflict handled by clap conflicts_with plus a defensive unreachable arm in main

## [4afb6db] Commit 4afb6db: spec model, validation, {file} interpolation, orchestrator flags

Implemented the full phase 0 spec model and orchestrator flags (commit 4afb6db). src/spec.rs carries the wire-format structs (deny_unknown_fields, kebab-case renames), hand-written deserializers for StrOrFile/StrListOrFile with precise messages (serde untagged errors were too cryptic for the exit-2 stderr contract), a normalized RunSpec with all defaults applied, semantic validation, and {file} interpolation relative to the spec file dir (cwd for argv/stdin, absolute paths never rebased). main.rs grew --task (validated via cuelib::head::validate_slug), --concurrency (u64, 0=unbounded), --timeout (nonzero). 55 tests total. Deviation from strict TDD ping-pong: the spec module's validation cases were written test-after in one slice rather than one-test-one-impl; CLI slices stayed strict red-green. Also noted: cargo fmt --all shows pre-existing drift in sibling crates (acuity/cue/cuelib/curator) — not touched, kept cue-agent-only formatting clean.

- **Found:** never-type coercion does not flow through unwrap_or_else(die) when die is generic over Display; closure |e| die(e) required (crates/cue-agent/src/main.rs:139)
- **Found:** workspace has no repo-wide rustfmt enforcement; only cue-agent is kept fmt-clean in this branch
- **Decided:** custom Deserialize impls over serde untagged for file-ref types: actionable error messages are part of the exit-2 contract
- **Decided:** strict consistency validation beyond the plan's minimum list: worktree base/name rejected under incompatible modes, session.id requires persist:true, zero timeouts rejected, env keys checked for empty/'='/NUL
- **Decided:** empty spec array is a usage error (exit 2)
- **Decided:** duplicate-id errors report first-seen index: spec[N] cross-reference
- **Open:** phase 2 run-id minting (run-N) must dodge explicit user ids that collide with the run-N namespace
- **Open:** cuelib tmp artifact helper gap still unverified (phase 1 item)

