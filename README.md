# cue

A file-based memory system for agentic workflows. See `.cue/master/spec/index.md`
for the project intent and current state.

This repository also ships the `acuity` binary: a stateless HTTP server that
receives `session.idle` events from an opencode plugin and forwards them to a
Gotify notification server.

## NixOS module

The flake exposes a NixOS module at `nixosModules.acuity`. Import it into your
system configuration to run
`acuity` as a managed systemd service:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    cue.url = "github:palekiwi-labs/cue";
  };

  outputs = { self, nixpkgs, cue, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        cue.nixosModules.acuity
        {
          services.acuity = {
            enable = true;
            gotifyUrl = "https://gotify.example.com";
            port = 33222;
            environmentFile = "/run/keys/acuity.env";
          };
        }
        # ...your other system modules...
      ];
    };
  };
}
```

### Environment file

The `environmentFile` must be readable by the `acuity` system user and contain
at least:

```
ACUITY_GOTIFY_TOKEN=<your-gotify-app-token>
```

This matches the read at `crates/acuity/src/main.rs:48`. The token is never
stored in the Nix store; load it via your secrets mechanism (e.g. `sops-nix`,
`agenix`, or a manually-managed `/run/keys/acuity.env`).

### Options

| Option | Type | Default | Description |
|---|---|---|---|
| `services.acuity.enable` | bool | `false` | Enable the service. |
| `services.acuity.package` | package | workspace build | acuity package to run. |
| `services.acuity.gotifyUrl` | string | `"http://localhost"` | Gotify base URL (no trailing slash). |
| `services.acuity.port` | port | `33222` | Listen port. |
| `services.acuity.environmentFile` | path | (required) | systemd EnvironmentFile providing `ACUITY_GOTIFY_TOKEN`. |
| `services.acuity.user` | string | `"acuity"` | System user. |
| `services.acuity.group` | string | `"acuity"` | System group. |

### Hardening notes

The module applies aggressive systemd hardening (`ProtectSystem=strict`,
`ProtectHome=true`, `PrivateTmp=true`, `MemoryDenyWriteExecute=true`, etc.).
Two things to be aware of:

- If you keep `environmentFile` under `/home/...`, override `ProtectHome` or
  move the file to `/run/keys/` or `/etc/`.
- `MemoryDenyWriteExecute=true` is enabled; verified safe for the rustls-tls
  backend via the live service smoke test. If you switch TLS backends, re-verify.
