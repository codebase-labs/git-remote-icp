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

          rust-httpd = craneLib.buildPackage {
            src = pkgs.fetchFromGitHub {
              owner = "PritiKumr";
              repo = "rust-httpd";
              rev = "dd9a6e4e5b6e2f177398ad8c0127d227891539cb";
              sha256 = "sha256-6J1yx5na0swUzs0ps9hXB32u+y7t240dX7m7vszjm4I=";
            };
          };

          git-remote-http-reqwest = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "http"; external = "http-reqwest"; };
            port = "8888";
            path_ = "/cgi/git-http-backend";
            installCheckInputs = [
              rust-httpd
            ];
            configure = ''
              git config --global http.receivePack true
            '';
            setup = ''
              mkdir test-repo-bare/cgi
              ln -s ${pkgs.git}/libexec/git-core/git-http-backend test-repo-bare/cgi/git-http-backend

              # Start HTTP server

              cd test-repo-bare

              GIT_HTTP_EXPORT_ALL=1 HTTP_GIT_PROTOCOL=version=2 rust-httpd &
              HTTP_SERVER_PID=$!

              trap "EXIT_CODE=\$? && kill \$HTTP_SERVER_PID && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              # Exit cleanly
              kill "$HTTP_SERVER_PID"
            '';
          };

          git-remote-icp = pkgs.callPackage ./nix/git-remote-helper.nix {
            inherit craneLib src;
            scheme = { internal = "http"; external = "icp"; };
            configure = ''
              git config --global --bool icp.fetchRootKey true
              git config --global icp.replicaUrl http://localhost:8000
              git config --global icp.canisterId rwlgt-iiaaa-aaaaa-aaaaa-cai
              git config --global icp.privateKey "$PWD/identity.pem"
            '';
            setup = ''
              exit 1
            '';
            teardown = ''
              exit 1
            '';
          };

          git-remote-tcp = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "git"; external = "tcp"; };
            # DEFAULT_GIT_PORT is 9418
            port = "9418";
            setup = ''
              # Start Git daemon

              # Based on https://github.com/Byron/gitoxide/blob/0c9c48b3b91a1396eb1796f288a2cb10380d1f14/tests/helpers.sh#L59
              git daemon --verbose --base-path=test-repo-bare --enable=receive-pack --export-all &
              GIT_DAEMON_PID=$!

              trap "EXIT_CODE=\$? && kill \$GIT_DAEMON_PID && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              # Exit cleanly
              kill "$GIT_DAEMON_PID"
            '';
          };

          apps = {
            rust-httpd = flake-utils.lib.mkApp {
              drv = rust-httpd;
            };

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
                rust-httpd
              ;
            };

            packages = {
              inherit
                git-remote-http-reqwest
                # git-remote-icp
                git-remote-tcp
                rust-httpd
              ;
            };

            inherit apps;

            # defaultPackage = packages.git-remote-icp;
            # defaultApp = apps.git-remote-icp;

            devShell = pkgs.mkShell {
              # RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
              inputsFrom = builtins.attrValues checks;
              nativeBuildInputs = pkgs.lib.foldl
                (state: drv: builtins.concatLists [state drv.nativeBuildInputs])
                []
                (pkgs.lib.attrValues packages)
              ;
            };
          }
      );
}
