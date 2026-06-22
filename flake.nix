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
        rustToolchain = fenix.packages.${system}.stable.toolchain;
      in
      let
        # Shared workspace build: compiles the full Cargo workspace once.
        # Both the `cue` and `acuity` binaries live in the same derivation.
        workspaceBuild = pkgs.rustPlatform.buildRustPackage {
          pname = "cue-workspace";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ rustToolchain pkgs.git ];

          # acuity links against libsqlite3 dynamically (sqlx sqlite feature
          # without the `bundled` flag).
          buildInputs = [ pkgs.sqlite ];

          meta = with pkgs.lib; {
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      in
      {
        # `cue` is the default package (memory / context CLI).
        packages.default = workspaceBuild // {
          meta = workspaceBuild.meta // {
            description = "cue: a file-based memory system for agentic workflows";
            mainProgram = "cue";
          };
        };

        # `acuity` is the observability ingestion server.
        packages.acuity = workspaceBuild // {
          meta = workspaceBuild.meta // {
            description = "acuity: observability ingestion server for the cue ecosystem";
            mainProgram = "acuity";
          };
        };

        devShells.default = pkgs.mkShell
          {
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
