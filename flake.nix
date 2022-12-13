{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    rust-overlay
  }:
    let
      supportedSystems = [
        flake-utils.lib.system.aarch64-darwin
        flake-utils.lib.system.x86_64-darwin
      ];
    in
      flake-utils.lib.eachSystem supportedSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              (import rust-overlay)
            ];
          };

          # rust = pkgs.rust-bin.stable.latest.default;
          rust = pkgs.rust-bin.nightly."2022-10-31".default;

          # NB: we don't need to overlay our custom toolchain for the *entire*
          # pkgs (which would require rebuilding anything else which uses rust).
          # Instead, we just want to update the scope that crane will use by appending
          # our specific toolchain there.
          craneLib = (crane.mkLib pkgs).overrideToolchain rust;
          # craneLib = crane.lib."${system}";

          src = ./.;

          # Build *just* the cargo dependencies, so we can reuse
          # all of that work (e.g. via cachix) when running in CI
          cargoArtifacts = craneLib.buildDepsOnly {
            inherit src;
            nativeBuildInputs = [
              pkgs.cmake
            ];
          };

          git-remote-http-reqwest = craneLib.buildPackage rec {
            pname = "git-remote-http-reqwest";
            inherit cargoArtifacts src;
            nativeBuildInputs = [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
            doInstallCheck = true;
            installCheckInputs = [
              pkgs.git
              pkgs.netcat
            ];
            installCheckPhase = ''
              echo "FIXME"
              exit 1
            '';
          };

          git-remote-tcp = pkgs.callPackage ./nix/git-remote-tcp.nix {
            inherit craneLib cargoArtifacts src;
          };

          apps = {
            git-remote-tcp = flake-utils.lib.mkApp {
              drv = git-remote-tcp;
            };
          };
        in
          rec {
            checks = {
              inherit
                git-remote-http-reqwest
                # git-remote-icp
                git-remote-tcp
              ;
            };

            packages = {
              inherit
                git-remote-http-reqwest
                # git-remote-icp
                git-remote-tcp
              ;
            };

            inherit apps;

            # defaultPackage = packages.git-remote-icp;
            # defaultApp = apps.git-remote-icp;

            devShell = pkgs.mkShell {
              # RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
              inputsFrom = builtins.attrValues self.checks;
              nativeBuildInputs = pkgs.lib.foldl
                (state: drv: builtins.concatLists [state drv.nativeBuildInputs])
                []
                ([cargoArtifacts] ++ pkgs.lib.attrValues packages)
              ;
            };
          }
      );
}
