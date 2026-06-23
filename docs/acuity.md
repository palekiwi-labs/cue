# acuity

`acuity` is the observability ingestion server for the cue ecosystem. It is an
HTTP server that persists agent lifecycle events (session idle, agent turns,
tool calls) to SQLite and optionally forwards notifications to a Gotify server.

It is deployed separately from `cue`/`curator` — typically on a server, not a
workstation.

## Install

```
nix run github:palekiwi-labs/cue#acuity
nix profile add github:palekiwi-labs/cue#acuity
```

## NixOS module

The flake exposes a NixOS module at `nixosModules.acuity`. Import it into your
system configuration to run `acuity` as a managed systemd service:

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

The `environmentFile` is **optional**. If provided, it must be readable by the
`acuity` system user. It is typically used to supply:

```
ACUITY_GOTIFY_TOKEN=<your-gotify-app-token>
```

The token is presence-based: when set, `session.idle` events are forwarded to
Gotify; when unset, the service still starts and persists events but skips
notifications (see `crates/acuity/src/main.rs:73-78`). The token is never
stored in the Nix store; load it via your secrets mechanism (e.g. `sops-nix`,
`agenix`, or a manually-managed `/run/keys/acuity.env`).

### Options

| Option | Type | Default | Description |
|---|---|---|---|
| `services.acuity.enable` | bool | `false` | Enable the service. |
| `services.acuity.package` | package | workspace build | acuity package to run. |
| `services.acuity.gotifyUrl` | string | `"http://localhost"` | Gotify base URL (no trailing slash). |
| `services.acuity.port` | port | `33222` | Listen port. |
| `services.acuity.dataDir` | path | `"/var/lib"` | Parent dir for the SQLite DB; binary appends `acuity/events.db`. |
| `services.acuity.environmentFile` | path | `null` | Optional systemd EnvironmentFile providing `ACUITY_GOTIFY_TOKEN`. |
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

## Non-NixOS Linux

NixOS is the only tested deployment path. On other Linux distributions you can
build from source:

```
cargo build --release -p acuity
```

`acuity` links `libsqlite3` dynamically (via `sqlx`), so the system's SQLite
development headers are required at build time. Configuration is entirely
environment-driven (see the Options table above for the variable names).

This path is **untested** — a hand-rolled service unit and a hardened process
supervisor are left to the operator. Non-NixOS users are welcome to contribute
and document a supported installation method.
