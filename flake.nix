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

          rust = pkgs.rust-bin.stable.latest.default;
          # rust = pkgs.rust-bin.nightly."2022-06-01".default;

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

            git config receive.denyCurrentBranch updateInstead
            # git config http.receivepack true
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
              pkgs.netcat
              pkgs.which
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

              # Based on https://github.com/Byron/gitoxide/blob/0c9c48b3b91a1396eb1796f288a2cb10380d1f14/tests/helpers.sh#L59
              git -c uploadPack.allowRefInWant daemon --verbose --base-path=${test-repo} --export-all --user-path &
              GIT_DAEMON_PID=$!

              trap "EXIT_CODE=\$? && kill \$GIT_DAEMON_PID && exit \$EXIT_CODE" EXIT

              # DEFAULT_GIT_PORT is 9418
              while ! nc -z localhost 9418; do
                sleep 0.1
              done

              git clone git://localhost/.git test-repo-tcp
              git clone ic::git://localhost/.git test-repo-ic

              GIT_LOG_TCP=$(git -C test-repo-tcp log)
              GIT_LOG_IC=$(git -C test-repo-ic log)

              if [ "$GIT_LOG_TCP" == "$GIT_LOG_IC" ]; then
                echo "GIT_LOG_TCP == GIT_LOG_IC"
              else
                echo "GIT_LOG_TCP != GIT_LOG_IC"
                exit 1
              fi

              GIT_DIFF_TCP=$(git -C test-repo-tcp diff)

              git -C test-repo-ic remote add -f test-repo-tcp "$PWD/test-repo-tcp"
              git -C test-repo-ic remote update
              GIT_DIFF_IC=$(git -C test-repo-ic diff main remotes/test-repo-tcp/main)

              if [ "$GIT_DIFF_TCP" == "$GIT_DIFF_IC" ]; then
                echo "GIT_DIFF_TCP == GIT_DIFF_IC"
              else
                echo "GIT_DIFF_TCP != GIT_DIFF_IC"
                exit 1
              fi

              kill "$GIT_DAEMON_PID"
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
                git-remote-ic
                test-repo
              ;
            };

            packages = {
              inherit
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
              nativeBuildInputs = cargoArtifacts.nativeBuildInputs ++ git-remote-ic.nativeBuildInputs ++ [
                pkgs.openssh
              ];
            };
          }
      );
}
