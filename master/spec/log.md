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

