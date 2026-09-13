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

        # NOTE: this sha256 covers the fetched toolchain components, not the
        # file itself. Renovate can bump `channel` in rust-toolchain.toml but
        # cannot update this hash, so a toolchain bump must be accompanied by
        # a manual hash refresh: set a fake hash, run `nix build`, and copy
        # the value from the `got:` line.
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-P30Tm3O7vQAE725YtDCDHGjNrSsfZO4us11UwJGZSJo=";
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Cargo.toml is the source of truth for the cue package version.
        cueCargo = pkgs.lib.importTOML ./crates/cue/Cargo.toml;

        common = {
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.git ];
          doCheck = false;
          meta = with pkgs.lib; {
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        # Builds only the codegen binary from acuity-schema. Exposed
        # publicly so consumers (e.g. cue-plugins) can run it directly
        # against their source tree via a `nix run` script.
        acuity-schema-codegen = rustPlatform.buildRustPackage (common // {
          pname = "acuity-schema-codegen";
          cargoBuildFlags = [ "-p" "acuity-schema" "--bin" "codegen" ];
        });
      in
      {
        # --- packages ---------------------------------------------------

        packages.default = self.packages.${system}.cue;

        packages.cue = rustPlatform.buildRustPackage (common // {
          pname = "cue";
          version = cueCargo.package.version;
          cargoBuildFlags = [ "-p" "cue" ];
          meta = common.meta // {
            description =
              "cue: file-based memory system for agentic workflows";
            mainProgram = "cue";
          };
        });

        packages.acuity = rustPlatform.buildRustPackage (common // {
          pname = "acuity";
          cargoBuildFlags = [ "-p" "acuity" ];
          buildInputs = [ pkgs.sqlite ];
          meta = common.meta // {
            description =
              "acuity: observability ingestion server for the cue ecosystem";
            mainProgram = "acuity";
          };
        });

        # `acuity-schema-types` invokes the codegen binary with $out as the
        # output directory and produces $out/types.ts — the TypeScript
        # discriminated union for all AcuityEvent variants. This is a
        # pre-built store artifact useful for CI or inspection. Consumers
        # that need to write types into their own source tree should use
        # `acuity-schema-codegen` directly instead.
        packages.acuity-schema-types = pkgs.runCommand "acuity-schema-types" { } ''
          mkdir -p $out
          ${acuity-schema-codegen}/bin/codegen $out
        '';

        # The codegen binary itself. Consumers run this directly to
        # generate types.ts into their source tree:
        #   nix run <cue-flake>#acuity-schema-codegen -- src/
        packages.acuity-schema-codegen = acuity-schema-codegen;

        # --- checks -----------------------------------------------------

        # Full workspace test suite via nextest. Run with:
        #   nix flake check
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
