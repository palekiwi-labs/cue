# Project Log

## [5a3ea36] acuity stateless MVP implemented

Areas 1-4 of phase-1-acuity-mvp.md complete. Two commits: cue workspace (5a3ea36) and cue-plugins (fc1373d).

- **Found:** reqwest default features pull in openssl-sys which fails in the Rust devshell (no pkg-config/openssl headers); fixed by using default-features = false + rustls-tls feature
- **Found:** cue-plugins had no Nix devshell; npm/bun not available in the Rust devshell
- **Found:** notification.ts was in auto-discovery glob path; moved to plugin/archive/ to decommission it
- **Decided:** Use reqwest rustls-tls to avoid native openssl dependency
- **Decided:** Add flake.nix to cue-plugins with bun devshell instead of Node/npm
- **Decided:** Plugin registered globally via absolute path in ~/.config/opencode/opencode.json
- **Open:** flake.lock for cue-plugins not yet generated (nix commands denied in Rust devshell -- needs manual: cd cue-plugins && nix flake lock)
- **Open:** Area 5 smoke tests and live session attestation still pending (human step)
- **Open:** bun install not yet run in cue-plugins (needs nix develop first)

## [5a3ea36] Fix acuity-plugin.ts: shape, types, configurable host

- **Found:** Plugin type is a function (PluginInput) => Promise<Hooks>, not a plain Hooks object
- **Found:** Client is not exported from @opencode-ai/sdk; correct type is Event for the event hook arg
- **Found:** session.get() returns { data: Session } not Session directly -- must unwrap .data
- **Found:** Session.title is string (non-optional); session?.title ?? null correctly produces string | null
- **Found:** tsc not runnable from Rust devshell (no node/bun); type-check must be done from cue-plugins devshell
- **Decided:** Use process.env.ACUITY_HOST ?? 'http://localhost:33222' so local runs work without config
- **Decided:** Add tsconfig.json with moduleResolution: bundler and noEmit: true for type-checking in devshell
- **Open:** Area 5 smoke tests and live session attestation still pending (human step)

## [5a3ea36-dirty] tsc passes clean in cue-plugins devshell

- **Found:** @opencode-ai/plugin shell.d.ts references Buffer and BufferEncoding from Node -- requires @types/node
- **Found:** process.env is available in Bun at runtime but needs @types/node for tsc to resolve it
- **Found:** bun install updated bun.lock (not bun.lockb -- Bun's newer text-format lockfile)
- **Decided:** Add @types/node to devDependencies and types:[node] in tsconfig.json
- **Decided:** nix develop --command pattern works for running devshell tools from the Rust devshell session

## [82a2f44] Opus consultation on review-fixes plan

Consulted opus on the review-fixes plan before implementation. Received substantive corrections on 6 items.

- **Found:** A4 default URL is wrong: 'http://localhost:80' should be 'http://localhost' (no port noise, no path); also needs trailing-slash normalization on load
- **Found:** A5 leaves a dead 'expected: String' binding that causes a compiler warning, breaking A13 clean-build gate -- must delete the old variable and rewrite the log branches
- **Found:** A7 leaves 'use std::sync::Arc' as an unused import -- must remove or A13 fails
- **Found:** A12 (narrow tokio features) is a fragile optimization that doesn't belong in a security-fix PR; reqwest/axum pull tokio features via cargo unification anyway
- **Found:** Part B order is inverted: deterministic handler tests (B3/B4/B5) should come BEFORE Part A so A5 refactor is guarded by tests
- **Found:** The 200 happy path (A2 token header, A4 URL) has zero test coverage -- A6/A7/A9 are main() rewrites that should be treated as one coherent pass, not four independent diffs
- **Found:** B1 needlessly adds serde_json to dev-dependencies -- it is already in dependencies and available to tests
- **Decided:** Add wiremock as dev-dependency and at least one happy-path test asserting X-Gotify-Key header is sent -- A2 and A4 must not ship blind
- **Decided:** Drop A12 (tokio feature narrowing) from this PR entirely -- not a review finding, out of scope
- **Decided:** Collapse A6/A7/A9 into a single coherent main() rewrite step in the plan
- **Decided:** Resequence: deterministic handler tests first (new Part A), then code fixes (Part B), so the schema refactor is guarded
- **Decided:** A10/B6: define concrete fallback for empty and root paths ('unknown'), assert single value in tests
- **Decided:** A4 trailing-slash normalization goes into Config::load() on the gotify_url field after load/extract
- **Open:** Verify X-Gotify-Key header name against the actual deployed Gotify version before merge

## [7d788ce] Part A complete: deterministic handler tests landed

Commit 7d788ce adds three tower::oneshot-based tests covering the deterministic paths (400 missing header, 400 wrong version, 422 malformed body). All three pass against the current implementation without any production code changes (only make_app() extraction to make the router testable). This establishes the safety net for the B5 schema-comparison refactor in Part B.

- **Decided:** Kept Arc<AppState> signature in make_app for Part A; Arc removal deferred to B6 with the rest of main() rewrite

## [39cbdb2] Part B complete: all code review fixes landed

Commit 39cbdb2 applies all blockers (B1 body limit, B2 token header), all 5 concerns (C1 comments, C2 gotify_url rename + trailing slash normalization, C3 u8 schema parse, C4 anyhow? instead of process::exit, C5 Arc removal), and all 4 nits (N1 unused dep, N2 no clone, N3 basename function, N4 structured tracing). All 3 Part A tests still green. Clippy clean. Build clean across workspace.

- **Decided:** basename() returns 'unknown' for empty strings and filesystem root; concrete contract chosen so the unit tests in C3 can assert a single value
- **Decided:** Comment about the ACUITY_GOTIFY_TOKEN / Config split lives in config.rs at the Env::prefixed call site (most discoverable); main.rs has a shorter pointer back
- **Decided:** while ends_with('/') pop() instead of trim_end_matches so all trailing slashes (not just one) are stripped

## [b45c634] Part C complete: review-fixes plan done, 11 tests green

Commit b45c634 closes the review-fixes plan. Final test count: 11 (3 deterministic handler, 2 wiremock-based forward-path, 6 basename edge cases). The two previously-untested security/behavior fixes (B2 X-Gotify-Key header, B4 gotify_url rename) are now guarded by the wiremock happy-path test that asserts on the header value.

- **Found:** wiremock MockServer::start().await returns a server whose .uri() gives the full http://127.0.0.1:PORT base URL -- exactly the shape gotify_url expects after the B4 rename
- **Found:** wiremock .expect(1) enforces exactly-one-request and fails the test if the handler's branching path skips the forward call (important for ensuring the 200 path actually reaches Gotify)
- **Found:** basename() being a private free function in main.rs is accessible from the tests child module without any pub annotation -- Rust's privacy model permits child-module access to parent-module items
- **Decided:** All three commits (Part A 7d788ce, Part B 39cbdb2, Part C b45c634) are independent and revertible -- if any single fix turns out wrong it can be reverted without unwinding the others

## [9dbfbce] 9dbfbce NixOS module for acuity landed on feat/acuity-mvp

Commit 9dbfbce adds a NixOS systemd module for the acuity service, exposing services.acuity.enable on NixOS hosts. Implements task/acuity-nixos-module.md on master. Design was grounded in a Gemini Flash consultation (5 questions, all recommendations accepted with one refinement).

- flake.nix restructured: outputs now destructures `self`, wraps eachDefaultSystem in parens, and merges `nixosModules.{default,acuity}` at the top level via `//`. Both module outputs evaluate `import ./nixos/module.nix self`.
- nixos/module.nix created with curry signature `self: { config, lib, pkgs, ... }:`. Options: enable, package (default self.packages.${pkgs.system}.default, with mandatory defaultText), gotifyUrl, port, environmentFile (required path), user, group. systemd unit uses ExecStart without CLI flags (crate is env-driven, unlike the reference) and injects config via `environment` (ACUITY_GOTIFY_URL, ACUITY_PORT) + EnvironmentFile (ACUITY_GOTIFY_TOKEN, matching main.rs:48). Flash's full hardening set applied.
- README.md created with a minimal consumer-flake example, options table, env-file format note, and ProtectHome/MemoryDenyWriteExecute caveats.

Pre-existing `cargo fmt --check` issues in crates/acuity/src/{config,main}.rs (import ordering, line wrapping) were left untouched -- they predate this work and are out of scope. Noted as a separate concern.

nix-side verification (nix flake check / build / eval / live smoke test) is deferred to human attestation -- nix commands are denied in the agent sandbox (only `nix develop *` is allowed). The task's 5 acceptance criteria are all human-attested. Criterion 5 specifically verifies that MemoryDenyWriteExecute=true does not crash the rustls-tls runtime; if it does, that single hardening flag should be dropped.

- **Found:** acuity is env-driven (no CLI flags), unlike the reference notifications-server which uses --hostname/--port/--notify-cmd/--gotify-host in ExecStart. The NixOS module therefore injects config via `environment` + `EnvironmentFile`, not ExecStart args.
- **Found:** ACUITY_GOTIFY_TOKEN is read directly via std::env::var in main.rs:48, NOT via figment Config -- intentional design to prevent silent shadowing. The EnvironmentFile must use this exact var name.
- **Found:** nix* commands are denied in the agent sandbox (only `nix develop *` allowed); all nix-side verification is necessarily human-attested.
- **Found:** nixosModules is a system-independent top-level output and cannot live inside flake-utils.eachDefaultSystem -- it must be merged via `//` (or any attrset-merge op) at the top level of `outputs`.
- **Found:** lib.mdDoc is deprecated in modern nixpkgs; plain strings are now the default rendering. Module uses plain strings throughout to be version-agnostic.
- **Found:** Pre-existing cargo fmt issues in crates/acuity/src/config.rs (import ordering) and crates/acuity/src/main.rs (line wrapping) predate this commit and are unrelated to the Nix module work.
- **Decided:** Branch: stay on feat/acuity-mvp (user considers the NixOS module part of MVP scope -- they need it to run acuity on their host)
- **Decided:** Package wiring: Flash's Hybrid A+B -- curried `self` provides default package, overrideable via `package` option, with mandatory `defaultText = lib.literalExpression` to keep doc-eval from crashing on undefined pkgs.system
- **Decided:** Module file location: nixos/module.nix (community convention for OS integration, scales to nixos/tests.nix later), not nix/acuity-module.nix
- **Decided:** Token option renamed from reference's gotifyTokenFile to environmentFile (nixpkgs idiom, e.g. services.grafana.environmentFile); file must contain ACUITY_GOTIFY_TOKEN=... to match main.rs:48; EnvironmentFile NOT prefixed with - so service fails loudly if file is absent
- **Decided:** Ship Flash's full hardening set day-one (user approved); MemoryDenyWriteExecute=true safety verified by the live smoke test (criterion 5)
- **Decided:** flake.nix restructure: keep flake-utils.eachDefaultSystem for packages/devShells, merge nixosModules via // at top level (minimal diff, no migration to forAllSystems)
- **Decided:** Curry pattern: module signature is `self: { config, lib, pkgs, ... }:` and flake does `import ./nixos/module.nix self` -- avoids _module.args.self wiring on the consumer side
- **Decided:** Pre-existing cargo fmt issues in crates/acuity/src/{config,main}.rs left untouched -- out of scope for this commit
- **Open:** Live smoke test (criterion 4 + 5): user must run systemctl start acuity on their NixOS host with an environmentFile providing ACUITY_GOTIFY_TOKEN, curl the events endpoint, and observe a Gotify notification. Confirms MemoryDenyWriteExecute=true is safe for rustls-tls.
- **Open:** nix flake check / nix build .#default / nix eval .#nixosModules.default -- all deferred to user (nix denied in agent sandbox).
- **Open:** CapabilityBoundingSet = "" semantics: depending on systemd version, empty value may either mean 'no caps' or 'reset to default'. Live smoke test will catch any issue. Most nixpkgs hardened services use this form successfully.
- **Open:** Pre-existing cargo fmt drift in crates/acuity/src/{config,main}.rs -- candidate for a separate cleanup commit.

## [e9126c7] e9126c7 rename nixos module, drop default alias

Commit e9126c7 (on top of 9dbfbce) renames the NixOS module file and drops the `nixosModules.default` alias, leaving `nixosModules.acuity` as the sole module output. The user proposed this refactor; I endorsed it and completed the migration by fixing the three stale references the user's original diff had missed.

- flake.nix: `nixosModules.default` dropped; `nixosModules.acuity` kept and repointed at `./nixos/acuity.nix`.
- nixos/module.nix -> nixos/acuity.nix: git detected the rename at 93% similarity (the 7% delta is the header-comment fix).
- nixos/acuity.nix:3-6: header comment rewritten -- removed the "default (also aliased as acuity)" framing.
- README.md:12-14,27: consumer example updated to import `cue.nixosModules.acuity`.
- task/acuity-nixos-module.md criterion 3: verify command updated from `.#nixosModules.default` to `.#nixosModules.acuity` (forward-looking instruction to the human verifier).

The task's Approach section still references `nixos/module.nix` and `nixosModules.default` as historical record of the original Flash-consulted plan. Not updated -- the rename is captured here instead, preserving the audit trail.

- **Found:** User's original diff was incomplete -- it updated flake.nix and renamed the file but left the module's own header comment and the README example referring to the dropped `default` alias. A consumer copy-pasting the README would have hit a broken import.
- **Found:** git rename detection scored the module.nix -> acuity.nix move at 93% similarity (the 7% delta being the comment fix), so history is preserved cleanly across the rename.
- **Found:** The cue artifacts (.cue/master/task/, .cue/feat-acuity-mvp/plan/, .cue/feat-acuity-mvp/spec/log.md) live in a separate git worktree -- grep against the main worktree does not surface them, and they are not part of this commit's scope by design (per cue skill: agents must not commit .cue/).
- **Decided:** Drop `nixosModules.default` alias entirely rather than keep it as a convenience. The cue workspace is multi-binary (cue, acuity, acuity-schema); `default = acuity` was an arbitrary 'primary service' frame. Named-only modules force consumer explicitness.
- **Decided:** Rename module.nix -> acuity.nix. Generic `module.nix` does not scale to a multi-service workspace -- parallel-named siblings (nixos/cue.nix, nixos/acuity.nix) are self-documenting at `ls nixos/`. Overrides Flash's single-module-repo convention, correctly, for this context.
- **Decided:** Update task criterion 3 verify command (forward-looking instruction to the human verifier) but leave the Approach section as historical record. The rename is bridged via this log entry.
- **Decided:** Fix the three stale references the user's original diff missed: nixos/acuity.nix:3-6 header comment, README.md:12-14 description sentence, README.md:27 import in consumer example.

## [e9126c7] Acuity stateless MVP + NixOS module fully complete

The acuity stateless MVP and its NixOS module are both fully complete and verified.

The acuity service now receives `session.idle` events from the opencode plugin, validates them against the versioned schema, and forwards notifications to Gotify via the `X-Gotify-Key` header.

The service is deployed on the user's `pale` host via a new NixOS module in the workspace flake, which applies aggressive systemd hardening (including `MemoryDenyWriteExecute=true`, verified safe for the rustls-tls backend).

Both the parent MVP task (`task/1781965432-d2f3251/acuity-stateless-mvp.md`) and the follow-up NixOS module task (`task/1782044318-b45c634/acuity-nixos-module.md`) are now marked `complete`. The old `notifications-server` has been decommissioned on the daily driver (`pale`).

Final state of acuity crate configuration (post-review fixes):
- `gotify_url`: configured base URL (default http://localhost)
- `port`: listen port (default 33222)
- `ACUITY_GOTIFY_TOKEN`: read directly from env for security

The `cue-plugins` repo has been updated with the vendored `types.ts` and the new `acuity-plugin.ts` which uses the standard Bun-global `fetch`.

This milestone closes Phase 1 of the acuity roadmap.

- **Found:** MemoryDenyWriteExecute=true is safe for the acuity service's rustls-tls runtime.
- **Found:** Acuity successfully forwards events to Gotify using the X-Gotify-Key header.
- **Decided:** All Phase 1 acuity MVP tasks and plans are marked complete.
- **Decided:** Acuity NixOS module task and plan are marked complete.
- **Decided:** The 'notifications-server' decommission criterion is considered satisfied by the migration of the daily driver ('pale').

