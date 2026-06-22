---
title: Add NixOS module for acuity service
status: complete
priority: high
---
# acuity NixOS module

Expose the `acuity` binary as a NixOS systemd module from the workspace flake so
the service can be enabled on a NixOS host with `services.acuity.enable = true;`.

This is part of the acuity MVP: without the module, the user cannot run `acuity`
on their NixOS host as a managed service.

## Source

- `.cue/feat-acuity-mvp/plan/phase-1-acuity-mvp.md` (parent MVP plan)
- `.cue/feat-acuity-mvp/spec/log.md` (acuity crate surface history)
- `.ref/notifications-server/flake.nix:80-169` (reference NixOS module)
- `crates/acuity/src/main.rs:48` (`ACUITY_GOTIFY_TOKEN` env var read)
- `crates/acuity/src/config.rs:8-20` (Config fields and defaults)

## Approach

Grounded in Gemini Flash consultation (recorded in branch log):

- New module file `nixos/module.nix` (community convention; scales to tests later).
- Hybrid package wiring: curried `self` provides the default `package`, with an
  overrideable option and a mandatory `defaultText` to keep doc-eval working.
- Token via `environmentFile` option (nixpkgs idiom); file must contain
  `ACUITY_GOTIFY_TOKEN=...` (matches `main.rs:48`).
- Aggressive systemd hardening (Flash's full set); `MemoryDenyWriteExecute=true`
  safety is verified by the live smoke test (criterion 5).
- `flake.nix` restructured so `nixosModules.default` / `nixosModules.acuity` live
  at the top level, merged via `//` outside `eachDefaultSystem`.
- README consumer-flake example.

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---|---|---|
| 1 | `nix flake check` passes (including doc-eval of `defaultText`) | human runs `nix flake check` on NixOS host | User attested: works (flake check passed) |
| 2 | `nix build .#default` produces a binary at `result/bin/acuity` | human runs `nix build .#default && ls result/bin/` | User attested: works (binary built) |
| 3 | `nix eval .#nixosModules.acuity` returns a well-formed module | human runs `nix eval .#nixosModules.acuity` | User attested: works (module well-formed) |
| 4 | Service starts under systemd and forwards a `session.idle` event to Gotify on the user's NixOS host | human attestation (`systemctl start` + curl + observe Gotify) | User (Jun 21 2026): works on pale |
| 5 | `MemoryDenyWriteExecute=true` hardening flag does not crash the rustls runtime | verified as part of criterion 4 (service stays up past boot) | User attested: safe on rustls |

Note: criteria 1-4 are human-attested because `nix` commands are denied in the
agent sandbox (only `nix develop *` is permitted). Criterion 5 piggybacks on 4:
if the service boots and serves a request under the full hardening set, MDWE is
confirmed safe for the rustls-tls backend.
