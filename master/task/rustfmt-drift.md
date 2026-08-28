---
title: Investigate rustfmt drift across workspace crates
status: open
priority: normal
kind: research
---
# Investigate rustfmt drift across workspace crates

## Problem

`cargo fmt --all -- --check` fails on crates acuity, cue, cuelib, and
curator (cue-agent, formatted 2026-08-24 with the current toolchain,
is clean). The repo therefore has no way to run a workspace-wide fmt
gate: any branch that touches sibling crates either reformats them
(noisy out-of-scope diffs) or leaves the workspace unformat-clean.

## Evidence

- Devshell toolchain is fenix `stable` (flake.nix), currently
  rustc 1.96 / rustfmt 2026-05; there is no rust-toolchain.toml pin.
- No CI exists (.github/workflows absent) and `nix flake check` runs
  tests only, so fmt compliance is never enforced.
- The diff shape matches rustfmt style-edition evolution rather than
  hand edits: import reordering to uppercase-first
  (`{bail, Context, Result}` -> `{Context, Result, bail}`), let-chain
  collapsing (`if x { if let ... }` -> `if x && let ...`), and
  one-lining of short if-else blocks.
- Precedent: commit 72b98a4 (2026-07-16) "fix: resolve clippy lints
  under newer toolchain" — the floating toolchain has already caused
  a similar class of breakage.

## Hypotheses to verify

1. Toolchain drift: files were last formatted by an older rustfmt;
   the current one applies edition-2024 style rules (import ordering,
   let-chains) that produce different output.
2. Style edition mismatch: crates declare edition 2024 but some
   formatting predates rustfmt honoring style_edition 2024 for them.

## Suggested steps

- Reproduce: `cargo fmt --all -- --check` and count diffs per crate.
- Decide the fix direction: one-shot `cargo fmt --all` commit
  (isolated, `style:` conventional commit), possibly combined with
  a pinned toolchain (rust-toolchain.toml or fenix pinned rev) to
  stop future drift.
- Optionally add a fmt check to CI or `nix flake check` so drift
  cannot accumulate silently again.

## Out of scope

- Reformatting itself (separate commit once direction is decided).
