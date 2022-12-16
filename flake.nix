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

          gemserv = craneLib.buildPackage {
            src = pkgs.fetchCrate {
              pname = "gemserv";
              version = "0.6.6";
              sha256 = "sha256-femZ68D480rgoaexUTzHDsu+0eluIvxoUoXElpgyqVA=";
            };
            nativeBuildInputs = [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
          };

          # https://portal.mozz.us/gemini/gmi.bacardi55.io/gemlog/2022/02/07/gemserv-update/
          # https://tildegit.org/solderpunk/gemcert
          gemserv-config = {
            interface = [
              "[::]:8888"
            ];
            log = "info";
            server = [
              {
                hostname = "localhost";
                dir = ".";
                key = pkgs.writeText "localhost-key" ''
                  -----BEGIN PRIVATE KEY-----
                  MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgQdw63IzKmLZGnghG
                  joG6tjY56fxnAbdFXGKmij3Fiu2hRANCAASD6oq7J0ZIZbMi7bBf6gaUvx9ZBTE3
                  TQAlB0/UXTWcukTsnbV4UcnyTnT184hwOrwRepT7tP+b6OYCgzLT+pjX
                  -----END PRIVATE KEY-----
                '';
                cert = pkgs.writeText "localhost-crt" ''
                  -----BEGIN CERTIFICATE-----
                  MIIBTDCB9KADAgECAhEApecIu8TIssPzNPb240JATDAKBggqhkjOPQQDAjAUMRIw
                  EAYDVQQDEwlsb2NhbGhvc3QwIBcNMjIxMjE2MDQzMDAzWhgPMjEyMjEyMTYwNDMw
                  MDNaMBQxEjAQBgNVBAMTCWxvY2FsaG9zdDBZMBMGByqGSM49AgEGCCqGSM49AwEH
                  A0IABIPqirsnRkhlsyLtsF/qBpS/H1kFMTdNACUHT9RdNZy6ROydtXhRyfJOdPXz
                  iHA6vBF6lPu0/5vo5gKDMtP6mNejJTAjMCEGA1UdEQQaMBiCCWxvY2FsaG9zdIIL
                  Ki5sb2NhbGhvc3QwCgYIKoZIzj0EAwIDRwAwRAIgL1GwQUa0S63nsepj5DyTVwG8
                  OPlCaUx72jY+Zet8gEgCIEeF0VCvXk5fCHFGqcdcOhICN5oCMTvDH2mCazYRkpGz
                  -----END CERTIFICATE-----
                '';
                cgi = true;
                cgipath = "/cgi-bin";
              }
            ];
          };

          gemserv-json = pkgs.writeText "gemserv-json" (builtins.toJSON gemserv-config);

          gemserv-toml = pkgs.runCommand "gemserv-toml" {
            buildInputs = [
              pkgs.remarshal
            ];
          } ''
            json2toml --input ${gemserv-json} --output $out
          '';

          git-remote-http-reqwest = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "http"; external = "http-reqwest"; };
            port = "8888";
            path_ = "/cgi-bin/git-http-backend";
            installCheckInputs = [
              gemserv
            ];
            configure = ''
              git config --global http.receivePack true
            '';
            setup = ''
              mkdir test-repo-bare/cgi-bin
              ln -s ${pkgs.git}/libexec/git-core/git-http-backend test-repo-bare/cgi-bin/git-http-backend

              # Start HTTP server

              cd test-repo-bare

              GIT_HTTP_EXPORT_ALL=1 HTTP_GIT_PROTOCOL=version=2 gemserv ${gemserv-toml} &
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
            gemserv = flake-utils.lib.mkApp {
              drv = gemserv;
            };

            git-remote-tcp = flake-utils.lib.mkApp {
              drv = git-remote-tcp;
            };
          };
        in
          rec {
            checks = {
              inherit
                gemserv
                git-remote-http-reqwest
                # git-remote-icp
                git-remote-tcp
              ;
            };

            packages = {
              inherit
                gemserv
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
