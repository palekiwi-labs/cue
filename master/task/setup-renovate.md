---
title: Set up Renovate bot
status: open
priority: normal
refs: .cue/master/task/release-0.2.0.md
kind: build
parent: release-0.2.0
tag: 0.2.0
---
Configure Renovate Bot for the `cue` repository to automate dependency maintenance for both Cargo crates and Nix flake inputs.

## Context & Objectives

- Automate dependency version bumps for Rust Cargo workspace (`cue`, `cuelib`, `curator`, `acuity`, `acuity-api`, `acuity-schema`) and Nix flake inputs (`nixpkgs`, `fenix`, `flake-utils`).
- Establish a synchronized schedule and grouping rules aligned with `cast` so that shared dependencies and flake toolchains stay in sync across the ecosystem.
- Configure `renovate.json` (or `.github/renovate.json`) with appropriate package rules (e.g. grouping minor/patch Rust crates, lockfile maintenance schedule, Nix flake input tracking).
- Ensure CI validation passes on Renovate PRs (runs `cargo test`, `cargo clippy`, and `nix flake check`).
