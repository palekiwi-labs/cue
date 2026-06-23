# Project Log

## [d2f3251] Created 8 roadmap tasks for the cue ecosystem

Created one task per roadmap phase following a design/discussion session with three rounds of Opus consultation. Tasks live in .cue/master/task/ across two timestamp directories.

- **Decided:** One task per roadmap phase (0-7)
- **Decided:** Phase 7 (hardening) created as a low-priority placeholder task flagged for future splitting
- **Decided:** Old stub tasks for curator and acuity were absent from the filesystem — no closing action needed

## [d2f3251] Promoted roadmap from trace to plan/index.md

The cue ecosystem roadmap was originally saved as a trace artifact during the design session. After review it was recognised as a living master plan rather than a point-in-time record, and promoted to plan/index.md (root). Task references added to each phase. The original trace is retained as historical record.

- **Decided:** Roadmap lives in plan/index.md as the master plan
- **Decided:** trace artifact retained as-is for historical record
- **Decided:** Each phase in plan/index.md now references its corresponding task file

## [858c351] Phase 0 complete -- feat/workspace-scaffold merged to master

- **Found:** buildRustPackage already compiles the full workspace via src = pkgs.lib.cleanSource ./.; no per-crate flake changes were needed for Phase 0
- **Found:** workspace-scaffold task status is complete; branch field should be cleared now that the PR is merged
- **Decided:** flake.nix intentionally left untouched for Phase 0; new crate Nix package outputs deferred until crates are shippable
- **Decided:** nix build acceptance criteria deferred; will be added as a dedicated tracked task rather than backfilled into existing open tasks

## [4f2fdc4] Acuity MVP Phase 1 merged to master

The `feat/acuity-mvp` branch has been merged into `master`, successfully closing Phase 1 of the acuity roadmap.

**Summary of deliverables:**
1. **Acuity Stateless MVP**: A Rust-based HTTP server that receives `session.idle` events from the opencode plugin, validates them against a versioned schema (`X-Acuity-Schema`), and forwards notifications to Gotify.
2. **NixOS Integration**: Added a dedicated NixOS module (`nixos/acuity.nix`) to the workspace flake, allowing the service to be managed via systemd with aggressive security hardening (including `MemoryDenyWriteExecute=true`, verified safe for the `rustls-tls` backend).
3. **Plugin Support**: Updated `cue-plugins` with the vendored `types.ts` and the `acuity-plugin.ts`, which replaces the legacy notification plugin.
4. **Verification**: 11 automated tests green, plus live deployment and smoke test on the daily driver host (`pale`).

The `acuity` binary is now the primary observability bridge for agentic workflows in the cue ecosystem. Phase 1 is officially complete.

- **Found:** MemoryDenyWriteExecute=true is safe for rustls-tls in the acuity service profile
- **Found:** Acuity successfully handles schema versioning and malformed payloads with appropriate HTTP status codes
- **Decided:** Merge feat/acuity-mvp to master
- **Decided:** Close Phase 1 of the acuity roadmap
- **Decided:** Standardize on named NixOS modules (services.acuity) instead of a generic .default alias

## [3ee4293] Phase 2 complete — curator MVP merged to master

The `feat/curator-mvp` branch has been merged into `master`, successfully closing Phase 2 (Artifact Kanban). 

**Summary of deliverables:**
1. **`cuelib` Artifact Reader**: Migrated and extended the artifact discovery and frontmatter parsing logic into `cuelib`. Added a typed `ArtifactMeta` reader with support for `TaskStatus` classification.
2. **`curator` TUI**: A functional three-column kanban board (Open | In Progress | Complete) that renders tasks from the CWD project's `.cue/master/task/` directory.
3. **TUI Navigation**: Implemented HJKL and Arrow key navigation, per-column scrolling, and active column highlighting with thick borders.
4. **Robustness & Safety**: Implemented terminal restoration on panic via a custom hook and adopted typed classification for artifacts to eliminate silent data loss from malformed status strings.

**Key Findings & Decisions:**
- **Found:** `cuelib`’s authoritative status logic (`is_kanban_visible`) can be reused in the TUI to ensure consistent artifact filtering without duplication.
- **Found:** A panic hook is superior to a simple RAII guard for terminal cleanup as it ensures the terminal is restored *before* the panic backtrace is printed.
- **Decided:** Retain the flat `App` state for the MVP; structural refactors to an array-based `[ColumnState; 3]` are deferred to later phases.
- **Decided:** `curator` remains read-only and CWD-only for Phase 2; multi-project support and mutation operations are sequenced for later.

Phase 2 is officially complete. `curator` now provides a stable, terminal-native view of a project's intent and progress.

- **Found:** `ArtifactMeta::status::<T>()` generic accessor simplifies classification across multiple artifact types.
- **Found:** Removing the unused `event-stream` feature from `crossterm` reduces transitive dependency weight (futures-core) in the `curator` binary.
- **Decided:** Merge feat/curator-mvp to master.
- **Decided:** Mark roadmap task `curator-artifact-kanban.md` as complete.

## [3309558] Phase 3 complete — acuity full event model merged to master

The `feat/acuity-full-event-model` branch was merged via PR #24, closing
Phase 3 of the acuity roadmap.

**Summary of deliverables:**
1. **4-event discriminated union** in `acuity-schema`: `AcuityEvent` with
   `SessionIdle`, `AgentTurnCompleted`, `ToolCallRequested`, `ToolCallCompleted`.
   Internally tagged (`#[serde(tag = "type", rename_all = "snake_case")]`),
   forward-compatible (no `deny_unknown_fields`).
2. **SQLite persistence** via `sqlx` in `acuity`: inline idempotent DDL
   (`CREATE TABLE IF NOT EXISTS`), `seq INTEGER PRIMARY KEY AUTOINCREMENT`
   as the future SSE resume cursor, `payload` column stores raw request
   bytes (faithful copy, not re-serialized).
3. **Gotify refactor**: presence-based opt-in (`ACUITY_GOTIFY_TOKEN`
   optional), persist-first / always-200 / fire-and-forget via `tokio::spawn`.
   Eliminated the Phase 1 double-persist class of bugs.
4. **NixOS module updates**: per-binary flake derivations, sqlite buildInput.
5. **Dependency hygiene**: dropped unused `macros` and `migrate` sqlx
   features after Opus consultation (commit db09029); 455 lines pruned
   from Cargo.lock.

138 workspace tests green; `cargo clippy --workspace -- -D warnings` clean.

- **Found:** In-memory SQLite test pools need max_connections(1) — each connection to :memory: gets an isolated database
- **Found:** sqlx::query() multi-statement strings work in 0.8 (driver iterates via prepare_next tail loop); deprecated path is style-only
- **Found:** ts-rs needs #[ts(export_to = "types.ts")] on the enum AND inner structs or types scatter to per-variant files
- **Decided:** AgentTurnStarted dropped — no curator view reads a start row; liveness belongs to Phase 5 SSE
- **Decided:** payload = raw request bytes (faithful copy); no deny_unknown_fields for forward-compat
- **Decided:** Workspace policy: feature flags follow call sites, not roadmaps. Required-to-compile plumbing exempt; speculative Phase-N bets fail.
- **Decided:** Merge feat/acuity-full-event-model to master; close Phase 3

## [300e744] Fixed acuity README drift against Phase 3 module

docs-only commit 300e744 on master. The README still described acuity in Phase 1 terms (stateless, session.idle-only, Gotify-forwarding, environmentFile required) while the module and binary had moved on in Phase 3. Fixed three drift points:

1. Intro (README.md:6-8): dropped "stateless"; acuity is now an HTTP ingestion server persisting lifecycle events (session idle, agent turns, tool calls) to SQLite with optional Gotify forwarding.
2. Environment file section (README.md:43-56): environmentFile is optional (presence-based token); fixed stale source citation main.rs:48 -> main.rs:73-78.
3. Options table (README.md:66-67): environmentFile default corrected from "(required)" to `null`; added the missing dataDir option (default /var/lib, events.db path).

No code changed; no build/test run needed (markdown only). Verified no residual "stateless"/"required"/main.rs:48 language via grep.

- **Found:** README.md:64 listed environmentFile as '(required)' but acuity.nix:56-72 made it optional (default null) during the Phase 3 Gotify refactor
- **Found:** The dataDir option (acuity.nix:41-54) added in Phase 3 was entirely absent from the README options table
- **Found:** Token presence logic lives at crates/acuity/src/main.rs:73-78; the old README cited main.rs:48 which no longer corresponds to the token read
- **Decided:** Scope the fix to the acuity/NixOS-module sections of the README only; leave the repo-wide project intro (lines 1-4) and hardening notes untouched since they remain accurate for Phase 3
- **Decided:** Cite the token-presence match arm main.rs:73-78 rather than just the var read at main.rs:74 so the README points at the semantic behavior, not just the line that loads the env var

## [ffab4fe] Rewrote README and extracted acuity docs to docs/

docs commit ffab4fe on master. Restructured the repo's documentation per user feedback that the README was dominated by acuity/NixOS-module content and linked into gitignored .cue/ artifacts.

Changes:
1. README.md near-total rewrite into 4 sections: (a) what the repo is -- in-place description, dead .cue link removed; (b) ASCII architecture diagram (terminal-renderable, no Unicode); (c) Nix quickstart for all installable packages (cue default + curator + acuity); (d) links into docs/.
2. docs/ created (flat): docs/acuity.md (full -- moved NixOS module content, env file, options table, hardening notes, + untested non-NixOS Linux caveat inviting contributions), docs/cue.md and docs/curator.md (terse stubs).

Confirmed via rg: no markdown links into .cue/ remain; no stale .cue/master references. docs-only change, no code/build impact. .cue excluded from the commit.

- **Found:** README.md:3 linked to .cue/master/spec/index.md which does not exist in the working tree and is gitignored (.gitignore:35); .cue/ is tracked only on the orphan cue/origin-cue branch
- **Found:** No docs/ directory existed prior to this commit
- **Found:** Flake exposes packages.default = cue plus curator and acuity outputs (flake.nix:52,65,78) -- all three are user-installable via nix profile add / nix run
- **Found:** Workspace has 6 crates but only 3 ship binaries (cue, curator, acuity); cuelib/acuity-schema/acuity-api are internal library/codegen crates not worth their own install docs
- **Decided:** README must never link into .cue/ -- those artifacts are gitignored on master and live on the orphan cue branch, so any such link is a 404 on GitHub
- **Decided:** Architecture diagram is plain ASCII (no Unicode box-drawing) so it renders in a terminal as well as on GitHub
- **Decided:** Nix quickstart covers all three installable flake outputs (cue default, curator, acuity), not just cue
- **Decided:** Non-NixOS Linux install for acuity is a brief suggestion flagged untested with a call for contributions, rather than a hand-rolled systemd unit -- NixOS remains the only tested path
- **Decided:** Flat docs/ layout (docs/acuity.md etc.) for now; nest later if per-topic subpages are needed
- **Decided:** Descriptive mentions of the runtime .cue/ directory are fine (e.g. docs/cue.md); the rule prohibits links to .cue/ artifact paths as repo content

## [8330d03] Phase 4 merged: cue-plugins full emitter live on master

The feat/cue-plugins-full-emitter PR has been merged. Phase 4 is complete and live on master.

What shipped:
- cue flake: packages.acuity-schema-codegen and packages.acuity-schema-types exposed publicly. Consumers pin cue as a flake input and run the codegen binary directly into their source tree via `nix run .#update-types`.
- cue-plugins: acuity-plugin.ts now emits all four AcuityEvent variants (SessionIdle, AgentTurnCompleted, ToolCallRequested, ToolCallCompleted) from live agent sessions. Types vendored under src/generated/acuity/ with linguist-generated gitattributes marking. Flake input wired (git+file:// during dev; TODO post-merge to switch to github:palekiwi-labs/cue).

Code review by gemini-3.5-flash and opus found a critical postEvent bug: fetch().catch() silently swallowed HTTP 4xx/5xx rejections. Fixed with try/catch + res.ok check before merge.

Live smoke test confirmed: 33 events across 3 sessions in acuity SQLite, all four types present with correct payloads (real token counts, tool metadata, session titles).

Deferred to Phase 7 hardening: dedup map unbounded growth on crashed sessions, multi-turn dedup reset semantics, server-side idempotency (UNIQUE constraint), retry/buffer for transient outages.

Next: Phase 5 — acuity-api read model (query API + SSE).

- **Found:** fetch() does not reject on HTTP 4xx/5xx — only network failures
- **Found:** nix store files are 0444; cp preserves mode, breaking re-runs
- **Found:** acuity server db.rs has no UNIQUE constraint — client dedup is sole defense against duplicates
- **Found:** SDK part.state.input appears empty at pending state in live testing — args may be in part.state.raw
- **Decided:** Nix flake input + writeShellScriptBin for type generation (no committed shell script)
- **Decided:** Generated types under src/generated/acuity/ with linguist-generated marking
- **Decided:** Expose codegen binary publicly — run directly into source tree
- **Decided:** try/catch + res.ok for postEvent error handling (not fetch().catch())
- **Open:** Phase 5: acuity-api read model — query API + SSE against real SQLite data
- **Open:** Phase 7: switch cue-plugins flake input from git+file:// to github:palekiwi-labs/cue
- **Open:** Phase 7: server-side idempotency (UNIQUE constraint + INSERT OR IGNORE)
- **Open:** Phase 7: dedup lifecycle improvements (LRU bound, session.deleted listener)

## [50aaa40] Phase 5 merged — acuity-api read model live on master

The feat/acuity-api-mvp branch has been merged into master, closing Phase 5 of the acuity roadmap.

Deliverables:
1. acuity-api crate: EventRecord, EventsPage (with explicit next_after pagination cursor), re-export of AcuityEvent so curator needs only one crate dependency.
2. GET /events: paginated historical query with after/limit/session_id/event_type params; DB errors surface as 500.
3. GET /events/stream: poll-based SSE (500 ms interval, drain-first inner loop bounded by SSE_MAX_DRAIN_PAGES=10, explicit 15 s keep-alive, defensive Last-Event-ID parsing).
4. Opus review + verification pass: all critical/major findings addressed. Explicit pagination cursor (next_after) fixes silent data-loss bug for limit > 500. DB-error 500 replaces silent empty-page swallowing.
5. Workspace-wide rustfmt applied (pre-existing debt cleaned up). Stale Cargo.lock synced. Unused serde_json dep dropped from acuity.
6. 35 acuity tests green; clippy -D warnings clean.

Deferred to Phase 7 (captured in master/todo/1782231988-7de1560/acuity-sse-deferred-followups.md):
- Broadcast-channel SSE redesign (poll sleep → notify recv, ~500 ms → ~0 ms latency)
- SSE disconnect / no-leak test
- Cosmetic nits (expect panic, redundant after clamp, 80-col lines)

Opus performance analysis (consulted post-merge decision): the broadcast redesign does NOT remove the bounded drain loop — only the sleep(500ms) is replaced by recv(). Lagged error from broadcast must be handled explicitly. Deferral is correct; gate on Phase 6 dogfooding signal.

- **Found:** gemini-flash subagents (consultant-gemini-flash, diff-reviewer-gemini-3.5-flash) return empty result bodies in this environment — a systemic issue; use opus/sonnet instead
- **Found:** The broadcast-channel SSE redesign does NOT remove the bounded drain loop or catch-up machinery — only the sleep(500ms) is replaced by broadcast recv(). The claim in the original todo that it removes drain machinery is incorrect.
- **Found:** tokio::sync::broadcast Lagged error must be handled explicitly when using () notification payloads — treat identically to a normal wake (drain from cursor)
- **Found:** DELETE journal mode (not WAL) in acuity SQLite means push-based SSE slightly increases the chance of a SELECT/commit race vs poll, not decreases it — journal mode is a separate concern orthogonal to poll-vs-push
- **Found:** The SSE disconnect/no-leak test (Major #4) is higher-value than the broadcast redesign as a pre-Phase-7 target — it covers a correctness gap regardless of loop strategy
- **Decided:** Merge feat/acuity-api-mvp to master; close Phase 5
- **Decided:** Defer broadcast-channel SSE redesign to Phase 7; gate on Phase 6 dogfooding signal (whether 500 ms latency is actually annoying in the curator TUI)
- **Decided:** If SSE effort is spent before Phase 7, prioritise the disconnect/no-leak test over the redesign
- **Decided:** Broadcast redesign implementation sketch recorded in the deferred todo for cheap pickup in Phase 7
- **Open:** Phase 6: curator live half — wire acuity-api SSE into the curator TUI activity feed

## [5be2d77] cue --dir / -C flag merged to master

The feat/cue-dir-flag branch has been merged into master.

Summary of deliverables:
1. Global --dir / -C flag on the cue CLI (clap global = true, accepted
   before or after the subcommand name). Mirrors the git -C convention.
2. Validation: single metadata() call resolves the path and verifies it
   is a directory; canonicalize() makes it absolute and resolves
   symlinks before passing to command handlers. No handler changes
   were needed — they all already take &cwd.
3. 8 integration tests covering: long flag targeting, -C alias,
   nonexistent path error, file-not-directory error, relative path
   resolution, non-git directory error, flag-after-subcommand
   (regression guard for global = true), and add mutation path.

Code review (consultant-opus): APPROVED with minor suggestions, all
addressed. One deferred item captured as a todo:
master/todo/1782236157-8b2d012/cross-binary-dir-flag-naming.md
(curator --root vs cue --dir naming inconsistency; address in Phase 6).

Original todo master/todo/1782211100-a8d0fb9/cue-support-dir-flag.md
closed as superseded by the task.

- **Found:** clap global = true is the correct attribute for a flag accepted in any position relative to the subcommand
- **Found:** metadata() follows symlinks, matching the semantics of the original exists()/is_dir() pair but in a single syscall
- **Found:** curator already has a --root flag (crates/curator/src/main.rs:22-23) with no validation that does the same conceptual thing as cue --dir — naming inconsistency to align later
- **Decided:** Merge feat/cue-dir-flag to master; close the cue-dir-flag task
- **Decided:** git -C convention (--dir long, -C short) chosen as the flag naming
- **Decided:** Defer curator --root vs cue --dir naming alignment to Phase 6; captured as a todo

