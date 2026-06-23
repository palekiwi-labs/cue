{
  description = "cue: a file-based memory system for agentic workflows";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix, flake-utils, ... }:
    (flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Wire fenix toolchain into rustPlatform explicitly rather than
        # injecting it via PATH. This ensures the fenix cargo/rustc are used
        # by all buildRustPackage hooks, not just shadowed on PATH.
        rustToolchain = fenix.packages.${system}.stable.toolchain;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Attributes shared across all per-binary derivations.
        # NOTE: if a derivation needs to extend nativeBuildInputs, use:
        #   nativeBuildInputs = common.nativeBuildInputs ++ [ extra ];
        # Never override the list outright — that silently drops pkgs.git.
        common = {
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          # pkgs.git is retained: some dep build scripts shell out to git
          # during `nix build` sandbox execution.
          nativeBuildInputs = [ pkgs.git ];
          # Tests run via the workspace-tests check below; skip per-package
          # check phases to keep `nix build` fast and avoid running acuity's
          # async sqlite test suite inside the sandbox.
          doCheck = false;
          meta = with pkgs.lib; {
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      in
      {
        # --- packages ---------------------------------------------------

        # `cue` is the default: the file-based memory CLI for workstations.
        packages.default = self.packages.${system}.cue;

        packages.cue = rustPlatform.buildRustPackage (common // {
          pname = "cue";
          cargoBuildFlags = [ "-p" "cue" ];
          meta = common.meta // {
            description =
              "cue: file-based memory system for agentic workflows";
            mainProgram = "cue";
          };
        });

        # `curator` is the TUI companion for the cue memory system.
        packages.curator = rustPlatform.buildRustPackage (common // {
          pname = "curator";
          cargoBuildFlags = [ "-p" "curator" ];
          meta = common.meta // {
            description = "curator: TUI for the cue memory system";
            mainProgram = "curator";
          };
        });

        # `acuity` is the observability ingestion server — deployed
        # separately from cue/curator (typically on a server, not a
        # workstation). Only this derivation needs libsqlite3; the others
        # have no sqlite dependency and must not carry it in their closure.
        packages.acuity = rustPlatform.buildRustPackage (common // {
          pname = "acuity";
          cargoBuildFlags = [ "-p" "acuity" ];
          # sqlx sqlite feature links libsqlite3 dynamically. Not bundled:
          # acuity is deployed via the NixOS module which pins the store
          # path, so there is no "missing system lib" failure mode.
          buildInputs = [ pkgs.sqlite ];
          meta = common.meta // {
            description =
              "acuity: observability ingestion server for the cue ecosystem";
            mainProgram = "acuity";
          };
        });

        # --- checks -----------------------------------------------------

        # Full workspace test suite via nextest. Run with:
        #   nix flake check
        # This covers all crates including cuelib, which would otherwise
        # fall through the cracks with per-crate -p scoping.
        checks.workspace-tests = rustPlatform.buildRustPackage (common // {
          pname = "cue-workspace-tests";
          # Tests need sqlite for the acuity in-crate test suite.
          buildInputs = [ pkgs.sqlite ];
          nativeBuildInputs = common.nativeBuildInputs
            ++ [ pkgs.cargo-nextest ];
          doCheck = true;
          buildPhase = "echo 'skipping build in test-only derivation'";
          checkPhase = ''
            cargo nextest run --workspace --locked
          '';
          installPhase = ''
            mkdir -p $out
          '';
        });

        # --- devShells --------------------------------------------------

        devShells.default = pkgs.mkShell {
          name = "cue";
          buildInputs = [
            rustToolchain
            pkgs.git
            pkgs.rust-analyzer
            pkgs.cargo-expand
            pkgs.cargo-watch
            pkgs.cargo-edit
            pkgs.cargo-nextest

            pkgs.sqlite
          ];

          shellHook = ''
            echo "Rust version: $(rustc --version)"
          '';
        };
      }))
    // {
      nixosModules.acuity = import ./nixos/acuity.nix self;
    };
}
