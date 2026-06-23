---
priority: high
title: Decide on flake outputs structure
status: complete
---

Single derivation (workspace build) vs separate derivations like in `cast`:

```nix
      {
        packages = {
          cast = pkgs.rustPlatform.buildRustPackage (common // {
            pname = "cast";
            cargoBuildFlags = [ "-p" "cast" ];
            cargoTestFlags = [ "-p" "cast" ];
            meta = with pkgs.lib; {
              description = "cast - coding agent sandbox tool";
              homepage = "https://github.com/palekiwi-labs/cast";
              license = licenses.mit;
            };
          });

          cast-mcp-client = pkgs.rustPlatform.buildRustPackage (common // {
            pname = "cast-mcp-client";
            cargoBuildFlags = [ "-p" "cast-mcp-client" ];
            cargoTestFlags = [ "-p" "cast-mcp-client" ];
            nativeCheckInputs = [ pkgs.bash pkgs.jq ];
            meta = with pkgs.lib; {
              description = "Lightweight MCP client for cast";
              homepage = "https://github.com/palekiwi-labs/cast";
              license = licenses.mit;
            };
          });

          default = self.packages.${system}.cast;
        };
```

