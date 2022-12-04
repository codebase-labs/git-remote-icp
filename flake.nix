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
              pkgs.cmake
            ];
          };

          git-remote-icp = craneLib.buildPackage rec {
            pname = "git-remote-icp";
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
              set -e

              export HOME=$TMP
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

              export GIT_AUTHOR_DATE="2022-11-14 21:26:57 -0800"
              export GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"

              git config --global init.defaultBranch main
              git config --global user.name "Test"
              git config --global user.email 0+test.users.noreply@codebase.org
              git config --global receive.denyCurrentBranch updateInstead


              # Set up test repo

              mkdir test-repo
              git -C test-repo init
              echo "# Hello, World!" > test-repo/README.md
              git -C test-repo add .
              git -C test-repo commit -m "Initial commit"


              # Start Git daemon

              # Based on https://github.com/Byron/gitoxide/blob/0c9c48b3b91a1396eb1796f288a2cb10380d1f14/tests/helpers.sh#L59
              git daemon --verbose --base-path=test-repo --enable=receive-pack --export-all --user-path &
              GIT_DAEMON_PID=$!

              trap "EXIT_CODE=\$? && kill \$GIT_DAEMON_PID && exit \$EXIT_CODE" EXIT

              # DEFAULT_GIT_PORT is 9418
              while ! nc -z localhost 9418; do
                sleep 0.1
              done


              # Test clone

              git clone git://localhost/.git test-repo-tcp
              git clone icp://localhost/.git test-repo-icp

              GIT_LOG_TCP=$(git -C test-repo-tcp log)
              GIT_LOG_ICP=$(git -C test-repo-icp log)

              if [ "$GIT_LOG_TCP" == "$GIT_LOG_ICP" ]; then
                echo "GIT_LOG_TCP == GIT_LOG_ICP"
              else
                echo "GIT_LOG_TCP != GIT_LOG_ICP"
                exit 1
              fi

              GIT_DIFF_TCP=$(git -C test-repo-tcp diff)

              git -C test-repo-icp remote add -f test-repo-tcp "$PWD/test-repo-tcp"
              git -C test-repo-icp remote update
              GIT_DIFF_ICP=$(git -C test-repo-icp diff main remotes/test-repo-tcp/main)

              if [ "$GIT_DIFF_TCP" == "$GIT_DIFF_ICP" ]; then
                echo "GIT_DIFF_TCP == GIT_DIFF_ICP"
              else
                echo "GIT_DIFF_TCP != GIT_DIFF_ICP"
                exit 1
              fi


              # Test push

              echo "\n" >> test-repo-tcp/README.md
              git -C test-repo-tcp add .
              git -C test-repo-tcp commit -m "Add trailing newline"
              git -C test-repo-tcp push origin main

              echo "\n" >> test-repo-icp/README.md
              git -C test-repo-icp add .
              git -C test-repo-icp commit -m "Add trailing newline"
              git -C test-repo-icp push origin main

              GIT_LOG_TCP_REMOTE=$(git -C test-repo-tcp log origin/main)
              GIT_LOG_ICP_REMOTE=$(git -C test-repo-icp log origin/main)

              if [ "$GIT_LOG_TCP_REMOTE" == "$GIT_LOG_ICP_REMOTE" ]; then
                echo "GIT_LOG_TCP_REMOTE == GIT_LOG_ICP_REMOTE"
              else
                echo "GIT_LOG_TCP_REMOTE != GIT_LOG_ICP_REMOTE"
                echo "<<<<<<< GIT_LOG_TCP_REMOTE"
                echo "$GIT_LOG_TCP_REMOTE"
                echo "======="
                echo "$GIT_LOG_ICP_REMOTE"
                echo ">>>>>>> GIT_LOG_ICP_REMOTE"

                exit 1
              fi

              GIT_DIFF_TCP_REMOTE=$(git -C test-repo-tcp diff origin/main origin/main)
              GIT_DIFF_ICP_REMOTE=$(git -C test-repo-icp diff origin/main remotes/test-repo-tcp/main)

              if [ "$GIT_DIFF_TCP_REMOTE" == "$GIT_DIFF_ICP_REMOTE" ]; then
                echo "GIT_DIFF_TCP_REMOTE == GIT_DIFF_ICP_REMOTE"
              else
                echo "GIT_DIFF_TCP_REMOTE != GIT_DIFF_ICP_REMOTE"
                echo "<<<<<<< GIT_DIFF_TCP_REMOTE"
                echo "$GIT_DIFF_TCP_REMOTE"
                echo "======="
                echo "$GIT_DIFF_ICP_REMOTE"
                echo ">>>>>>> GIT_DIFF_ICP_REMOTE"

                exit 1
              fi


              # Exit cleanly

              kill "$GIT_DAEMON_PID"
            '';
          };

          apps = {
            git-remote-icp = flake-utils.lib.mkApp {
              drv = git-remote-icp;
            };
          };
        in
          rec {
            checks = {
              inherit
                git-remote-icp
              ;
            };

            packages = {
              inherit
                git-remote-icp
              ;
            };

            inherit apps;

            defaultPackage = packages.git-remote-icp;
            defaultApp = apps.git-remote-icp;

            devShell = pkgs.mkShell {
              # RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
              inputsFrom = builtins.attrValues self.checks;
              nativeBuildInputs = cargoArtifacts.nativeBuildInputs ++ git-remote-icp.nativeBuildInputs ++ [
                pkgs.openssh
              ];
            };
          }
      );
}
