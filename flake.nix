{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    adobe-git-server-src = {
      url = "github:adobe/git-server?rev=3509b1c6e4db64075b62324912054944e1603986";
      flake = false;
    };

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    npmlock2nix = {
      url = "github:nix-community/npmlock2nix";
      flake = false;
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };

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
    adobe-git-server-src,
    crane,
    flake-utils,
    npmlock2nix,
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
              (final: prev: {
                # We're stuck on nodejs-14_x while using npmlock2nix, but
                # temporarily switching to nodejs-16_x with its support for
                # lockfile v2 can be useful to catch incompatible
                # dependency issues that nodejs-14_x and v1 doesn't.
                # https://github.com/nix-community/npmlock2nix/issues/153
                nodejs = pkgs.nodejs-14_x;
                npmlock2nix = pkgs.callPackage npmlock2nix {};
              })
              (import rust-overlay)
            ];
          };

          rust = pkgs.rust-bin.stable.latest.default;
          # rust = pkgs.rust-bin.nightly."2022-06-01".default;

          # NB: we don't need to overlay our custom toolchain for the *entire*
          # pkgs (which would require rebuidling anything else which uses rust).
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
              # For git-transport http-client-curl
              pkgs.cmake
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];
          };

          test-repo = pkgs.runCommand "test-repo" {
            buildInputs = [
              pkgs.git
            ];
          } ''
            HOME=$TMP

            mkdir $out
            cd $out

            git config --global init.defaultBranch main
            git config --global user.name "Test"
            git config --global user.email 0+test.users.noreply@codebase.org

            git init
            echo "# Hello, World!" > README.md
            git add .
            git commit -m "Initial commit"
          '';

          adobe-git-server = pkgs.npmlock2nix.build {
            src = adobe-git-server-src;
            buildCommands = [];
            installPhase = ''
              mkdir $out
              cp -R . $out
            '';
          };

          adobe-git-server-config = {
            virtualRepos = {
              "test-owner" = {
                "test-repo" = {
                  path = test-repo;
                };
              };
            };
            listen = {
              http = {
                port = 4887;
              };
            };
          };

          adobe-git-server-config-js = pkgs.writeText "config.js" ''
            module.exports = ${builtins.toJSON adobe-git-server-config};
          '';

          git-remote-ic = craneLib.buildPackage rec {
            pname = "git-remote-ic";
            inherit cargoArtifacts src;
            nativeBuildInputs = [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
            doInstallCheck = true;
            installCheckInputs = [
              pkgs.git
              pkgs.nodejs
            ];
            installCheckPhase = ''
              set -e

              export PATH=$out/bin:$PATH

              export RUST_BACKTRACE=full
              export RUST_LOG=trace

              export GIT_TRACE=true
              export GIT_CURL_VERBOSE=true
              export GIT_TRACE_PACK_ACCESS=true
              export GIT_TRACE_PACKET=true
              export GIT_TRACE_PACKFILE=true
              export GIT_TRACE_PERFORMANCE=true
              export GIT_TRACE_SETUP=true
              export GIT_TRACE_SHALLOW=true

              cp ${adobe-git-server-config-js} config.js
              node ${adobe-git-server}/index.js &
              NODE_PID=$!

              trap "EXIT_CODE=\$? && kill \$NODE_PID && exit \$EXIT_CODE" EXIT

              sleep 1

              git clone http://localhost:${builtins.toJSON adobe-git-server-config.listen.http.port}/test-owner/test-repo.git test-repo-http
              git clone ic::http://localhost:${builtins.toJSON adobe-git-server-config.listen.http.port}/test-owner/test-repo.git test-repo-ic

              diff --recursive test-repo-http test-repo-ic

              kill "$NODE_PID"
            '';
          };

          apps = {
            git-remote-ic = flake-utils.lib.mkApp {
              drv = git-remote-ic;
            };
          };
        in
          rec {
            checks = {
              inherit
                adobe-git-server
                adobe-git-server-config-js
                git-remote-ic
                test-repo
              ;
            };

            packages = {
              inherit
                adobe-git-server
                adobe-git-server-config-js
                git-remote-ic
                test-repo
              ;
            };

            inherit apps;

            defaultPackage = packages.git-remote-ic;
            defaultApp = apps.git-remote-ic;

            devShell = pkgs.mkShell {
              # RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
              inputsFrom = builtins.attrValues self.checks;
              nativeBuildInputs = cargoArtifacts.nativeBuildInputs ++ git-remote-ic.nativeBuildInputs;
            };
          }
      );
}
