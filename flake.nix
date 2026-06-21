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
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "cue";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ rustToolchain pkgs.git ];

          buildInputs = [];

          meta = with pkgs.lib; {
            description = "cue: a file-based memory system for agentic workflows";
            license = licenses.mit;
            maintainers = [ ];
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
