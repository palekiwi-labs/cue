---
title: Update dependencies for 0.2.0
status: inbox
priority: normal
refs: .cue/master/task/release-0.2.0.md
kind: build
parent: release-0.2.0
---
Update direct Cargo dependencies and Nix flake inputs across the `cue` monorepo ahead of the 0.2.0 release, aligning shared dependencies with `cast`.

## Context & Scope

- Audit and update direct dependencies across workspace crates:
  - `cue/crates/cue/Cargo.toml`
  - `cue/crates/cuelib/Cargo.toml`
  - `cue/crates/curator/Cargo.toml`
  - `cue/crates/acuity/Cargo.toml`
  - `cue/crates/acuity-api/Cargo.toml`
  - `cue/crates/acuity-schema/Cargo.toml`
- Synchronize heavy shared dependencies with `cast` (e.g. `reqwest`, `tokio`, `serde`, `clap`, `axum`, `tracing`, `figment`, `dirs`) to maximize compilation caching and minimize duplication.
- Update `flake.lock` (`nix flake update`) to align `nixpkgs` and `fenix` toolchains with `cast` for unified Nix store paths.
- Ensure all tests (`nix flake check`, `cargo nextest run --workspace`, `cargo clippy`) pass cleanly.