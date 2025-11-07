{
  description = "BGG API gateway";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
    # Until https://github.com/NixOS/nixpkgs/pull/414495
    #nixpkgs-unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    mkElmDerivation.url = "github:jeslie0/mkElmDerivation";
  };
  outputs =
    {
      self,
      nixpkgs,
      # nixpkgs-unstable,
      flake-utils,
      mkElmDerivation,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (
          import nixpkgs {
            overlays = [ mkElmDerivation.overlays.mkElmDerivation ];
            inherit system;
          }
        );

        runDeps = with pkgs; [
          openssl
        ];

        buildDeps =
          with pkgs;
          [
            pkg-config
          ]
          ++ runDeps;
      in
      {
        packages =
          let
            version = "0.1.0";

          in
          {
            bgg-api = pkgs.rustPlatform.buildRustPackage rec {
              crateName = "bgg-api";

              src = ./backend;

              name = "${crateName}-${version}";

              outputs = [
                "out"
                "migrations"
              ];

              cargoLock.lockFile = backend/Cargo.lock;

              nativeBuildInputs = buildDeps;

              buildInputs = buildDeps;

              checkFlags = "--skip db::";

              postInstall = ''
                mkdir -p $migrations
                cp migrations/* $migrations
              '';

              meta = with pkgs.lib; {
                description = "A JSON API gateway for BGG";
                longDescription = ''
                  BGG has some API set up, but it needs local caching.
                  This service both handles that, API authentication, and translates the XML to JSON-LD.
                '';
                homepage = "https://crates.io/crates/bgg-api";
                license = licenses.mpl20;
                maintainers = [ maintainers.nyarly ];
              };
            };
          };
        nixosModules.bgg-api =
          {
            config,
            lig,
            pkgs,
            ...
          }@params:
          {
            options = import nix/backend/options.nix self.packages.${system} params;
            config = import nix/backend/config.nix params;
          };
        devShells.default =
          # if you don't what to use Nix, here are the dependencies you need:
          pkgs.mkShell {
            buildInputs =
              with pkgs;
              [
                cargo
                cargo-expand
                rustc
                rust-analyzer
                clippy

                process-compose
                watchexec
                postgresql_15
                sqlx-cli
                biscuit-cli
                mailpit
                openssl
              ]
              ++ buildDeps; # If you're doing your own installs, you can ignore this
          };
      }
    );
}
