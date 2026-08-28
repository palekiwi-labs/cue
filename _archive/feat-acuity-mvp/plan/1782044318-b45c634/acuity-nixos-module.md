---
status: complete
---

## Foreword

This executive plan adds a NixOS systemd module for the `acuity` service to the
`cue` workspace flake, on the current `feat/acuity-mvp` branch (the user
considers this part of the MVP scope -- they need it to run `acuity` on their
NixOS host).

It implements `task/acuity-nixos-module.md` on master. The design is grounded
in a Gemini Flash consultation (recorded in the branch log):

- Hybrid package wiring: curried `self` provides the default `package`, with an
  overrideable option and a mandatory `defaultText` so doc-eval does not crash
  on undefined `pkgs.system`.
- Module file at `nixos/module.nix` (community convention for OS-level
  integration; scales to `nixos/tests.nix` later).
- Token via `environmentFile` option (nixpkgs idiom, e.g.
  `services.grafana.environmentFile`); file must contain `ACUITY_GOTIFY_TOKEN=...`
  (matches `crates/acuity/src/main.rs:48`).
- Aggressive systemd hardening (Flash's full set); `MemoryDenyWriteExecute=true`
  safety is verified by the live smoke test (Phase 5).
- README consumer-flake example.

Prerequisites already met:
- `acuity` binary exists in the workspace (`crates/acuity/`).
- Current `flake.nix` builds the whole workspace via `packages.default`, so
  `${packages.default}/bin/acuity` already exists.
- `feat/acuity-mvp` is the active branch.

Constraint: `nix*` commands are denied in the agent sandbox (only
`nix develop *` is allowed). All nix-side verification (criteria 1-3 of the
task) is deferred to human attestation in Phase 5.

## Steps

### Phase 1 -- restructure flake outputs

- [x] **1.1** Edit `/home/pl/code/palekiwi-labs/cue/flake.nix`:
  - Add `self` to the `outputs` argument destructure.
  - Wrap the existing `flake-utils.lib.eachDefaultSystem (...)` body in parens.
  - Merge a top-level `nixosModules` attrset via `//`.
  - `nixosModules.default` and `nixosModules.acuity` both evaluate
    `import ./nixos/module.nix self`.

### Phase 2 -- module file

- [x] **2.1** Create `/home/pl/code/palekiwi-labs/cue/nixos/module.nix` with:
  - Curry signature: `self: { config, lib, pkgs, ... }:`
  - `let cfg = config.services.acuity;` and the user/group defaults.
  - `options.services.acuity`:
    - `enable` (`mkEnableOption`)
    - `package` (`types.package`, default `self.packages.${pkgs.system}.default`,
      `defaultText = lib.literalExpression "self.packages.\${pkgs.system}.default"`)
    - `gotifyUrl` (`types.str`, default `"http://localhost"`)
    - `port` (`types.port`, default `33222`)
    - `environmentFile` (`types.path`, required -- no default; documents that
      the file must contain `ACUITY_GOTIFY_TOKEN=...`)
    - `user` (`types.str`, default `"acuity"`)
    - `group` (`types.str`, default `"acuity"`)
  - `config = lib.mkIf cfg.enable`:
    - Conditional system user/group creation (gated on default name, like
      reference `flake.nix:132-138`).
    - `systemd.services.acuity`:
      - `description`, `wantedBy = ["multi-user.target"]`,
        `after = ["network.target"]`
      - `serviceConfig`:
        - `Type = "exec"`, `User = cfg.user`, `Group = cfg.group`
        - `ExecStart = "${cfg.package}/bin/acuity"` (no CLI flags -- crate is
          env-driven, unlike the reference)
        - `Restart = "on-failure"`, `RestartSec = "5s"`
        - `EnvironmentFile = cfg.environmentFile`
        - Flash's full hardening set: `ProtectSystem = "strict"`,
          `ProtectHome = true`, `PrivateTmp = true`, `PrivateDevices = true`,
          `ProtectKernelTunables = true`, `ProtectKernelModules = true`,
          `ProtectControlGroups = true`,
          `RestrictAddressFamilies = ["AF_INET" "AF_INET6"]`,
          `NoNewPrivileges = true`, `CapabilityBoundingSet = ""`,
          `RestrictNamespaces = true`, `RestrictRealtime = true`,
          `MemoryDenyWriteExecute = true`, `LockPersonality = true`
      - `environment`:
        - `ACUITY_GOTIFY_URL = cfg.gotifyUrl`
        - `ACUITY_PORT = toString cfg.port`
        - `RUST_LOG = "info"`

### Phase 3 -- documentation

- [x] **3.1** Add a "NixOS module" section to `/home/pl/code/palekiwi-labs/cue/README.md`
  (create the file if missing) with a minimal consumer-flake example:
  - `inputs.cue.url = ...`
  - `nixosConfigurations.<host>` block with `modules = [ cue.nixosModules.default ... ]`
  - `services.acuity = { enable = true; environmentFile = "/run/keys/acuity.env"; gotifyUrl = ...; };`
  - Note that the env file must contain `ACUITY_GOTIFY_TOKEN=...` (one key/value
    per line, standard systemd `EnvironmentFile` format).

### Phase 4 -- agent-side verification

- [x] **4.1** `cargo build --workspace` -- confirm nothing on the Rust side
  regressed. (The flake change should not affect this; verifying anyway.)

### Phase 5 -- human-attested verification (gated on user)

- [x] **5.1** Human runs `nix flake check` and reports pass/fail.
- [x] **5.2** Human runs `nix build .#default && ls result/bin/acuity`.
- [x] **5.3** Human runs `nix eval .#nixosModules.default` (well-formed module).
- [x] **5.4** Human runs `systemctl start acuity` on their NixOS host with an
  `environmentFile` providing `ACUITY_GOTIFY_TOKEN`, then curls the events
  endpoint and observes a Gotify notification. Confirms `MemoryDenyWriteExecute`
  does not crash rustls (criterion 5).

### Phase 6 -- commit and log

- [x] **6.1** Stage only `flake.nix`, `nixos/module.nix`, `README.md` (NOT
  anything under `.cue/`).
- [x] **6.2** Commit with message `feat: add NixOS module for acuity service`.
- [x] **6.3** `cue log add` recording the milestone and the Flash consultation
  decisions.
