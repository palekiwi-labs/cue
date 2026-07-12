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

## [5be2d77] Research complete: log --branch flag

- **Found:** Task premise is partially wrong: `log list` ALREADY supports --branch (cli.rs:172-176); only `log add` lacks it (cli.rs:151-170)
- **Found:** `log add` hardcodes the current git branch in add_entry (log/mod.rs:52-56) and LogAddOptions carries no branch field (log/mod.rs:22-24)
- **Found:** Pattern to follow is `add --branch` (cli.rs:53-55 + add/mod.rs:47-57); change spans 3 source files + 1 test, ~10-15 prod lines
- **Found:** No master-branch special-casing in CLI; writing to master log works for free once --branch is wired
- **Found:** Shared sanitize_branch_name helper exists in cuelib (git.rs:141-143) but add/list/log inline .replace() instead — minor DRY duplication, optional refactor only
- **Open:** Short-flag choice for log add --branch: match `add` (-b) or match log list (long-only). Recommend -b.

## [be00e7e] feat: cue log add --branch shipped

The `--branch` / `-b` flag is now available on `cue log add`, allowing log entries to be written to an arbitrary branch (e.g. master) instead of only the current git branch. This entry is written using that feature.

- **Found:** sanitize_branch_name helper already existed in cuelib but was unused by add/log paths
- **Decided:** Use if let Some(b) pattern (matches add/mod.rs) not combinators
- **Decided:** Field named branch_name in LogAddOptions for consistency with AddOptions
- **Decided:** Sanitization via git::sanitize_branch_name helper, applied after resolution
- **Open:** validate_branch_name to reject .. path traversal (deferred todo)
- **Open:** Detached HEAD silently routes to HEAD/ directory (deferred todo)

## [ecd8396] [ecd8396] Phase 6 complete — curator live half (SSE + Activity/Diagnostics views) merged to master

The `feat/curator-w-acuity` branch was merged via PR #29, closing Phase 6 of the acuity roadmap. `curator` now consumes the acuity SSE stream and renders two new live views alongside the existing kanban.

**Architecture (Opus-reviewed, unified-channel variant of Option A):** Two leaf threads — crossterm input + tokio SSE — both post into a single `std::sync::mpsc::SyncSender<Msg>` (capacity 4096). The main loop blocks on `rx.recv()`, drains pending via `try_recv()`, then draws once. No async runtime in the main thread; zero borrow/cancellation hazards.

**Deliverables:**
1. **Msg enum** (`msg.rs`): `Input(Action)`, `Redraw`, `Sse(EventRecord)`, `SseStatus(SseStatus)`. Action extended with `SwitchView(View)` and `Refresh`.
2. **Input thread** (`input.rs`): blocking `crossterm::event::read()` loop; keys 1/2/3 switch views, `r` reloads.
3. **SSE thread** (`sse.rs`): single-threaded tokio runtime; `LineBuffer` is a pure synchronous SSE parser (chunk-boundary safe, keep-alive `:` comments skipped, cursor tracking, LF-join for multi-line `data:` per SSE spec). `next_backoff` pure helper caps at 5000ms.
4. **App state** (`app.rs`): `View` enum, `AcuityStatus`, ring buffer (`EVENT_CAP=2000`, oldest-first eviction), incremental `SessionSummary` HashMap updated *before* eviction so totals (project_dir, token sums, error counts) survive ring-buffer churn. `classify_tasks` extracted + priority-sorted (critical→high→normal→low).
5. **Three-view UI** (`ui.rs`): Kanban (priority-sorted), Activity Feed (reverse-chrono with session-group headers), Diagnostics (tool_call_* filter, errors in red). Status bar colour-codes SSE state.
6. **CLI**: `--acuity-url` / `$ACUITY_URL` (clap `env` feature); kanban-only mode when unset (sends `SseStatus::Disabled` synchronously).
7. **29 curator tests**, 138 workspace tests green; `cargo clippy --workspace -- -D warnings` clean.

**Review fixes (pre-merge, from Phase 6 code review):** M3 removed `eprintln!` from SSE thread (TUI corruption); M1 hoisted `reqwest::Client` to `run_loop` (built once, reused across reconnects); H2 fixed `data:` multi-line join per SSE spec + new test; C1 switched `Msg::Sse` to `try_send` so SSE bursts cannot starve the input thread or Quit; H1 added `drop(tx)` in main so `recv()` Err arm is reachable + removed dead trailing `Ok(())`. Follow-up Opus review confirmed all 5 fixes ship-able; one misleading M3 comment was clarified (05eeca7).

Three Opus review nits deferred (not merge blockers): C1 cursor-coupling note (drops are permanent across reconnects since cursor advances at parse time); H2 test contract comment (data: splits only safe on structural JSON boundaries); M1 optional `Client::builder().build()` for TLS-error symmetry.

- **Found:** Incremental SessionSummary HashMap is load-bearing — fold-on-render is silently wrong once old SessionIdle events age out of the ring buffer; updating the map before eviction preserves totals (project_dir, token sums, error counts)
- **Found:** SSE parsing must line-buffer across chunk boundaries AND skip : keep-alive comments or reconnect storms occur every ~15s when acuity emits its keep-alive
- **Found:** LineBuffer extracted as a pure synchronous struct enables full unit-testing of chunk-boundary/keep-alive/cursor/malformed-JSON logic with zero server infrastructure — 11 tests green on first run
- **Found:** try_send for Msg::Sse protects input/Quit liveness: when SSE burst saturates the channel, events are dropped (telemetry is lossy, ring buffer already caps UI at 2000) but the input thread never blocks. SseStatus stays blocking send() — rare and UI-critical
- **Found:** lb.feed() advances cursor at parse time, before try_send — so Full drops permanently skip the dropped event on reconnect (Last-Event-ID resumes after it). Correct lossiness model for live telemetry
- **Found:** clap env feature must be explicitly enabled to support #[arg(env = "ACUITY_URL")] — was not on by default
- **Found:** Cursor must be checked for LoopControl::Quit on BOTH the initial rx.recv() and the try_recv() drain — checking only the drain misses Quit from the first message in a batch
- **Found:** Connected status is sent inside connect_and_stream right after the HTTP 200 (before stream consumption) so the UI shows connected during active streaming, not just after stream end
- **Decided:** Merge feat/curator-w-acuity to master; close Phase 6
- **Decided:** Adopted unified-channel Option A (two leaf threads → single mpsc) over a multi-channel or async-main approach — zero borrow/cancellation hazards, instant keyboard response, batches SSE bursts
- **Decided:** Incremental SessionSummary updates before ring-buffer eviction (not fold-on-render) — the key correctness fix from the Opus architecture review
- **Decided:** LineBuffer + next_backoff extracted as pure synchronous helpers — enables Slice 8 unit tests without any server/async infrastructure
- **Decided:** try_send for Msg::Sse only; SseStatus remains blocking send() (low-frequency, UI-critical)
- **Decided:** Diagnostics view deserializes payload per-row (only tool_call_* events) — acceptable cost since filter is cheap and list is bounded by EVENT_CAP
- **Decided:** Server-side ?limit_history=N (Slice 2 follow-up) adopted as the only feasible cold-start replay bound given acuity's forward-only query API — deferred to a dedicated task; C1 try_send remains load-bearing until then
- **Decided:** Tier 3 TcpListener end-to-end test and TestBackend snapshot tests skipped — negative ROI per plan; Tier 1+2 cover ~90% of risk
- **Open:** Manual QA checklist (Slice 7) requires live acuity — cursor-resume correctness (step 7) cannot be fully covered by automated tests
- **Open:** Slice 2: server-side ?limit_history=N parameter to bound cold-start history replay — captured as todo on the feature branch
- **Open:** Phase 7 hardening: broadcast-channel SSE redesign, dedup lifecycle, server-side idempotency, curator --root vs cue --dir naming alignment

## [ecd8396] Research: Rust markdown chunking crates

- **Found:** text-splitter v0.32.0 (2026-06-16) is the dominant crate with 1.5M downloads and an explicit markdown feature backed by pulldown-cmark
- **Found:** pulldown-cmark v0.13.4 has 111M downloads and is the standard CommonMark parser; used by text-splitter
- **Found:** chunkedrs v1.0.4 (2026-06-07) explicitly says markdown-aware, recursive splitting, optional semantic mode via embeddings
- **Found:** niblits v0.3.14 (2026-06-22) is the largest library by source (5.9K lines), multi-format token-aware splitting
- **Found:** chunk v0.10.2 (2026-05-28, chonkie-inc) has 12K downloads, SIMD-accelerated, but markdown support is unconfirmed
- **Found:** markdown-chunk v0.1.0 (2026-05-16) is heading-hierarchy-aware, zero deps, but only 53 lines and 15 downloads
- **Found:** markdown-rag v0.1.0 (2026-05-29) is RAG-focused markdown splitter, alpha quality
- **Found:** markdown_splitter v0.1.1 (2020) is annotation-based, abandoned, not semantic
- **Found:** md-scatter v0.1.2 (2025-12-23) is a file-level split/reassemble tool, not an in-process chunking library
- **Found:** transmutation v0.3.3 (2026-06-18) converts 27 document formats for LLM ingestion, heavier scope
- **Found:** memchunk (chonkie-inc) appears superseded by chunk from the same org
- **Found:** document-splitter does NOT exist on crates.io (404)
- **Decided:** text-splitter is the only production-ready option for markdown-aware chunking in Rust as of June 2026

## [ecd8396] Rust RAG ecosystem research completed

- **Found:** rig-core v0.39.0 (updated 2026-06-19): actively maintained, 12+ vector store integrations, 1.35M downloads, used in production by St. Jude, Neon, etc.
- **Found:** langchain-rust v4.6.0 (last updated 2024-10-06): likely stalled, full RAG pipeline, supports Qdrant/Postgres/SQLite/SurrealDB
- **Found:** chonkie v0.1.1 exists as a Rust crate (ported from Python by chonkie.ai team, 729 downloads, very new)
- **Found:** llm-chain v0.13.0 (last updated 2023-11-15): abandoned, had Qdrant support
- **Found:** candle-core v0.11.0 (updated 2026-06-26): HuggingFace ML inference framework, not a RAG framework
- **Found:** qdrant-client v1.18.0: NO embedded mode, requires external server
- **Found:** lancedb v0.30.0: embedded/serverless vector DB, used in rig
- **Found:** sqlite-vec v0.1.9: in-process SQLite extension for vector search
- **Found:** usearch v2.25.3: single-file in-process vector search
- **Found:** fastembed v5.17.2: local embedding generation via ONNX, used by both rig and langchain-rust
- **Found:** pgvector v0.4.2: 14.1M downloads, PostgreSQL vector extension client
- **Decided:** rig is the leading Rust RAG/LLM framework as of 2026
- **Decided:** Qdrant has no embedded mode - always requires external server
- **Decided:** LanceDB is the best embedded vector store option for Rust

## [dab157e] Curator UX improvement plan finalised — 8 tasks created

Design session with user + Opus consultation produced a final roadmap for the curator improvements initiative. All artifacts created on master branch under timestamp 1782644149-dab157e.

- **Found:** Opus flagged tool-grouping by positional adjacency as wrong — must key on (session_id, turn_id)
- **Found:** All existing test fixtures use turn_id: t1 — a (session_id, turn_id) HashMap key prevents the latent collision this creates
- **Found:** SessionSummary.project_dir currently only set from SessionIdle — push_event must set it from every event after Phase 1
- **Found:** Plugin header is a hardcoded string literal not a variable — plugin is an external repo deployed separately
- **Decided:** Defer harness_version — not needed for prototype
- **Decided:** Defer schema versioning / backward compat enforcement — prototyping phase
- **Decided:** Add project_dir and harness to all four event structs (required on every event)
- **Decided:** project_dir stored as DB column for server-side queries AND on EventRecord for render path — tripling accepted, captured as low-priority todo
- **Decided:** Default fold state: all turns folded (addresses too-many-rows pain point)
- **Decided:** Tool-call grouping keyed on (session_id, turn_id) HashMap — not positional adjacency (Opus catch)
- **Decided:** Logical-identity selection tracking (not raw visual index) — survives fold/unfold and SSE arrivals
- **Decided:** Zero selected projects = show all (narrowing filter model)
- **Decided:** build_activity_items as pure function called at render time — no derived-state cache
- **Decided:** 8 implementation slices: 1 schema, 2 DB/EventRecord, 3 server filter, 4 plugin, 5 ActivityItem model, 6 activity view, 7 navigation, 8 projects view
- **Open:** harness_version availability from opencode SDK — investigate if needed later
- **Open:** Multi-project kanban — deferred to a later phase
- **Open:** Hostname field — deferred pending CAST_HOSTNAME injection

## [dab157e] Phase 7 of ecosystem-mvp deferred — curator UX improvements take priority

Phases 0-6 of plan/ecosystem-mvp.md are complete. Phase 7 (auth/trust boundary, SQLite retention, second harness, multi-host config) is being explicitly deferred. The pivot is to plan/1782644149-dab157e/curator-improvements.md, which addresses critical UX gaps in curator that emerged from daily use post-Phase 6.

- **Decided:** Phase 7 of plan/ecosystem-mvp.md is deferred indefinitely; it is not cancelled, only deprioritised below the curator improvements initiative
- **Decided:** plan/1782644149-dab157e/curator-improvements.md is the active plan; its 8 slices (schema fields, DB columns, server filter, plugin, ActivityItem model, activity view, navigation, projects view) replace Phase 7 as the next body of work
- **Decided:** plan/ecosystem-mvp.md status remains open rather than complete, as Phase 7 work is still owed; it must not be promoted to complete until hardening is addressed

## [dd6f86b] feat/curator-improvements-schema merged to master

Slices 1-3 of the curator improvements plan are merged. The branch delivered: project_dir + harness on all four acuity-schema event structs (+ accessors), DB layer columns + column-mismatch startup guard + EventFilter struct, project_dir filter on GET /events, and a post-merge fix for three missed test fixture constructors in curator (app.rs, ui.rs, sse.rs). Workspace fully green at merge.

- **Decided:** Slice 4 (cue-plugins) is next — add project_dir and harness to all four event payload objects in acuity-plugin.ts

## [325b779] [0371769] feat(acuity-plugin): Slice 4 complete — project_dir + harness in plugin

acuity-plugin.ts updated: SessionIdle gains harness: "opencode"; AgentTurnCompleted, ToolCallRequested, and ToolCallCompleted gain both project_dir: directory and harness: "opencode". tsc --noEmit clean. Committed to cue-plugins dev branch at 0371769.

- **Found:** SessionIdle already had project_dir: directory from a prior iteration — only harness was missing
- **Found:** directory is already in the plugin closure at line 65, no new variable needed
- **Decided:** harness hardcoded to "opencode" — this plugin is opencode-specific by definition

## [55d53fb] feat/curator-activity-item merged — rich activity feed with lineage, two-pane UX, and per-session caching

PR #32 merged to master. The feat/curator-activity-item branch built a rich activity feed in the curator TUI that surfaces per-session turns, tool calls, and lineage metadata (parent session, agent, model, title) flowing through the pipeline: opencode-plugin (TypeScript) -> acuity server (Rust/axum, SQLite) -> curator TUI (Rust/ratatui, SSE).

The branch shipped 6 slices across ~40 commits (2 repos), going from a flat event list with 8-char session-id truncation to a gitui-inspired two-pane TUI with columnar session rows, a three-state layout, a session Info block, and O(1) per-session data access via cached aggregates.

Slice 5 — ActivityItem enum + build_activity_items (ae09f03, 0c2ff37):
- New crates/curator/src/activity.rs module with ActivityItem enum (SessionHeader, Turn, Standalone) and build_activity_items pure function.
- Turn-folding scaffolding for stage C nested tree (currently unwired — its (session_id, project_dir) header key is degenerate since project_dir is workspace-constant).
- 7 unit tests covering Opus-flagged failure modes.

Slice 6a — SessionUpdated collection + logging + hardening:
- Added SessionUpdated schema variant (7f4bac5) carrying parent_id, agent, model, title, harness, project_dir — the lineage envelope.
- Curator SessionSummary ingest (1acf52f): parent_id set-once-if-some, title/agent/model last-writer-wins-when-Some.
- Plugin session.created/session.updated handlers (8555572 cue-plugins) with defensive casts (SDK 1.17.11 Session type still omits agent?/model?).
- Acuity dual-layer tracing (369a0d1): stderr at INFO, optional file at DEBUG with raw-vs-parsed delta.
- Plugin dedup (db90d51 cue-plugins): persistent lastSessionSig Map collapsed session_updated from 5+ identical rows to 1-2 per session.
- Model capture from AssistantMessage (3aeb835 cue-plugins, abf7772 + f5c1695 cue): added model: Option<String> to AgentTurnCompleted, populated from typed providerID/modelID fields — resolves the per-turn model for all sessions including subagents.
- Curator model ingest (d590d61): last-writer-wins from both SessionUpdated and AgentTurnCompleted.

Slice 6b — Stage-B rendering (cfa450e + 6 step commits):
- Killed the 8-char session-id prefix truncation (ses_0f14 collision); replaced with last-8-char suffix via session_label helper.
- Title-flip: dim id-suffix placeholder -> bright title when session_updated arrives — the live verification that 6a title-capture round-trips to the UI.
- Per-turn model on turn rows from AgentTurnCompleted.model.
- Hid session_updated rows entirely (payload already absorbed into SessionSummary before render); selection indexes the filtered set, mirroring Diagnostics.
- Fixed scroll_down_activity clamp (was against events.len(), now activity_len).
- activity.rs documented as stage-C scaffolding (build_activity_items unwired).

Slice 6c — Two-pane activity view (1ce0a99):
- Sessions pane (left 1/3) + Events pane (right 2/3) replacing the flat reverse-chrono list.
- Identity-based session selection (sel_session_id: Option<String>) — cursor follows the session, not the visual index. New sessions prepend at top without shifting existing selection.
- Event selection stays index-based (sel_activity: usize), resets to 0 on session change.
- sorted_sessions() filters by visible events, sorts first_seen desc with session_id asc tiebreak.
- +18 net tests (60 -> 78 curator tests).

Slice 6d — gitui-inspired UX (84908d6, 7ef8690, 27dceba, 633522a + QA polish c798df4, 54ddc6c, 4a2d9bb):
- Columnar session rows: harness (2 wide) | datetime (10 wide, local TZ) | project (20 wide, trunc_pad) | title (fill).
- Three-state ActivityLayout enum (SessionsFull / Split / DetailFull) replacing ActivityPane + pane_expanded bool.
- Navigation: Enter toggles SessionsFull<->Split; Right arrow -> DetailFull; Escape -> Split. Tab/z removed.
- Detail pane: Session Info block (title, id, project, agents, models, parent, tokens in/out, errors) + Events list below.
- Datetime helpers (format_datetime_on, format_event_datetime) convert UTC to host local TZ via chrono.
- harness_abbrev (oc/cc/pi/??), trunc_pad, format_tokens (comma separators).
- Colors: project=Magenta, harness=Blue, datetime=LightCyan, title=White+Bold (titled) / DarkGray (placeholder).
- highlight_symbol removed from activity lists (reclaimed 2 chars/row).

Slice 6e — Review fixes (c6d3932, 61ee148, 1839275, 92605bc):
- Clamped stale sel_activity after partial ring-buffer eviction (ensure_session_selection else-branch).
- Cached visible_event_count, unique_agents, unique_models on SessionSummary — eliminated three O(N) hot paths: session_event_len O(N)->O(1), sorted_sessions O(N*CAP)->O(N), session_unique_agents/models per-frame ring scan + serde_json::from_str -> O(1) Vec clone.
- Hoisted Local::now() from per-row to once-per-frame via format_datetime_on(ts, today).
- Removed dead is_active=true branch in render_sessions_pane.
- Consultant-opus review: SHIP. Cache invariant proven airtight.

Final state: 106 curator tests, full workspace green, clippy clean across all crates.

- **Found:** opencode session ids share a per-run prefix (ses_0f14...) so the old .get(..8) prefix-truncation caused 5 distinct sessions to render identically — the suffix-based session_label (last ~8 chars prefixed with ellipsis) is the fix
- **Found:** opencode fires session.created + multiple session.updated for the same state — the plugin handler emitted SessionUpdated for each with no dedup, producing 5+ identical rows per session. Fixed with persistent lastSessionSig Map keyed by sessionID over [parent_id, agent, model, title] signature
- **Found:** model is always null on SessionUpdated for Task-tool subagent sessions — they are created with no model (task.ts:129-145); the model is resolved per-turn at runtime. The reliable source is AssistantMessage.providerID/modelID (required typed fields), captured in the message.updated handler
- **Found:** SDK 1.17.11 Session type STILL omits agent?/model? — defensive casts in the SessionUpdated handler must stay
- **Found:** Session.model runtime shape is { id, providerID } NOT { modelID, providerID } — the previous cast used modelID which was a latent bug (always produced null model)
- **Found:** session_updated rows are pure noise in the activity feed: push_event absorbs their payload into SessionSummary synchronously before render, so hiding them entirely is correct
- **Found:** activity.rs:build_activity_items uses a (session_id, project_dir) header key that is degenerate (project_dir workspace-constant) — stage C reworks with parent_id-based nesting
- **Found:** app.sessions HashMap never shrinks (push_event only inserts) — sessions with all events evicted still appear; sorted_sessions must filter by visible_event_count > 0
- **Found:** The ensure_session_selection_is_idempotent test was itself testing broken behavior (sel_activity=5 for a 1-event session was an out-of-range index) — corrected to use a valid index
- **Found:** push_turn_at test helper bypasses push_event (pushes directly to app.events) and needed manual visible_event_count increment to stay compatible with the cached session_event_len
- **Decided:** Model capture goes on AgentTurnCompleted (not SessionUpdated) — model is per-turn; AgentTurnCompleted fires from message.updated which carries typed modelID/providerID
- **Decided:** SessionUpdated.model is kept (not removed) — cooperates with AgentTurnCompleted.model via curator last-writer-wins
- **Decided:** Dedup uses persistent lastSessionSig Map (not the turn/call dedup Map which is cleared on idle) — collapses identical consecutive session events across idle boundaries
- **Decided:** Two-pane layout over single-pane grouping: index-based selection is materially worse under first_seen-sorted grouping (new session inserts whole group, shifting every index)
- **Decided:** Session selection is identity-based (sel_session_id: Option<String>); event selection stays index-based (sel_activity: usize) — acceptable for stage B since shifts are within one session only
- **Decided:** Cache derived per-session aggregates (visible_event_count, unique_agents, unique_models) on SessionSummary rather than computing at render-time — SessionSummary is explicitly designed to survive ring-buffer eviction
- **Decided:** unique_agents/unique_models use Vec<String> with .contains() dedup (not HashSet) — n is tiny (1-5), preserves first-seen order, simpler API
- **Decided:** Removed format_datetime wrapper rather than marking #[allow(dead_code)] when it became unused after splitting to format_datetime_on
- **Decided:** activity.rs left unwired with doc-comments explaining stage-C nesting — it is turn-folding scaffolding, not a competing grouping engine for the flat session list
- **Open:** Stage C: nested tree with parent_id-based nesting, folding child sessions under parent turns; wire build_activity_items into Pane 2 with a session_id filter parameter
- **Open:** event_type column collapse — deferred to observe rendering in practice
- **Open:** 4 deferred master tasks: db-sessions-table-normalization, collect-user-turn-messages, improve-session-data-loading (per-session ring-buffer eviction + startup N-session fetch), clipboard-copy-session-id (needs arboard or xclip dependency)
- **Open:** acuity CorpusChanged event as future evolution target for acumen graph sync

