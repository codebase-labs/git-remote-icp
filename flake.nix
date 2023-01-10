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
        flake-utils.lib.system.x86_64-linux
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
          # rust = pkgs.rust-bin.nightly."2022-10-31".default;

          # NB: we don't need to overlay our custom toolchain for the *entire*
          # pkgs (which would require rebuilding anything else which uses rust).
          # Instead, we just want to update the scope that crane will use by appending
          # our specific toolchain there.
          craneLib = (crane.mkLib pkgs).overrideToolchain rust;
          # craneLib = crane.lib."${system}";

          src = ./.;

          lighttpd-conf = port: pkgs.writeText "lighthttpd.conf" ''
            server.document-root = var.CWD
            server.port = ${toString port}

            server.modules = (
              "mod_alias",
              "mod_setenv",
              "mod_cgi",
            )

            debug.log-request-header = "enable"
            debug.log-response-header = "enable"
            debug.log-file-not-found = "enable"
            debug.log-request-handling = "enable"
            debug.log-condition-handling = "enable"
            debug.log-condition-cache-handling = "enable"
            debug.log-ssl-noise = "enable"
            debug.log-timeouts = "enable"

            alias.url += (
              "/git" => "${pkgs.git}/libexec/git-core/git-http-backend"
            )

            $HTTP["url"] =~ "^/git" {
              cgi.assign = ("" => "")

              setenv.set-environment = (
                "GIT_PROJECT_ROOT" => var.CWD,
                "GIT_HTTP_EXPORT_ALL" => "",
                "REMOTE_USER" => "$REDIRECT_REMOTE_USER",
              )

              $REQUEST_HEADER["Git-Protocol"] =~ "(.+)" {
                setenv.add-environment += (
                  "HTTP_GIT_PROTOCOL" => "%1",
                )
              }
            }
          '';

          git-remote-helper = features: craneLib.buildPackage rec {
            inherit src;
            pname = "git-remote-helper";
            cargoExtraArgs = "--package ${pname} --features ${features}";
            cargoArtifacts = craneLib.buildDepsOnly {
              inherit cargoExtraArgs pname src;
            };
            nativeBuildInputs = [
              pkgs.cmake
            ];
          };

          git-remote-helper-async = git-remote-helper "async-network-client";
          git-remote-helper-blocking = git-remote-helper "blocking-network-client";

          git-remote-http-reqwest = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "http"; external = "http-reqwest"; };
            path_ = "/git/test-repo-bare";
            port = 8888;
            installCheckInputs = [
              pkgs.lighttpd
            ];
            configure = ''
              git config --global --type bool http.receivePack true
            '';
            setup = ''
              lighttpd -f ${lighttpd-conf port} -D 2>&1 &
              HTTP_SERVER_PID=$!
              trap "EXIT_CODE=\$? && kill \$HTTP_SERVER_PID && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              kill "$HTTP_SERVER_PID"
            '';
          };

          git-remote-icp = pkgs.callPackage ./nix/git-remote-helper.nix {
            inherit craneLib src;
            scheme = { internal = "http"; external = "icp"; };
            port = 1234;
            nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
              # https://nixos.wiki/wiki/Rust#Building_the_openssl-sys_crate
              pkgs.openssl_1_1
              pkgs.pkgconfig
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
            # This package is currently not tested _in this repository_
            doInstallCheck = false;
            configure = ''
              HOME=.
            '';
            setup = ''
            '';
            teardown = ''
            '';
          };

          git-remote-tcp = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "git"; external = "tcp"; };
            # DEFAULT_GIT_PORT is 9418
            path_ = "/test-repo-bare";
            port = 9418;
            setup = ''
              # Based on https://github.com/Byron/gitoxide/blob/0c9c48b3b91a1396eb1796f288a2cb10380d1f14/tests/helpers.sh#L59
              git daemon --verbose --base-path=. --enable=receive-pack --export-all &
              GIT_DAEMON_PID=$!
              trap "EXIT_CODE=\$? && kill \$GIT_DAEMON_PID && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              kill "$GIT_DAEMON_PID"
            '';
          };

          apps = {
            git-remote-http-reqwest = flake-utils.lib.mkApp {
              drv = git-remote-http-reqwest;
            };

            git-remote-icp = flake-utils.lib.mkApp {
              drv = git-remote-icp;
            };

            git-remote-tcp = flake-utils.lib.mkApp {
              drv = git-remote-tcp;
            };
          };
        in
          rec {
            checks = {
              inherit
                git-remote-helper-blocking
                git-remote-helper-async
                git-remote-http-reqwest
                git-remote-icp
                git-remote-tcp
              ;
            };

            packages = {
              inherit
                git-remote-helper-blocking
                git-remote-helper-async
                git-remote-http-reqwest
                git-remote-icp
                git-remote-tcp
              ;
              lighttpd-conf = lighttpd-conf 8888;
            };

            inherit apps;

            defaultPackage = packages.git-remote-icp;
            defaultApp = apps.git-remote-icp;

            devShell = pkgs.mkShell {
              # RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
              inputsFrom = builtins.attrValues checks;
              nativeBuildInputs = pkgs.lib.foldl
                (state: drv: builtins.concatLists [state drv.nativeBuildInputs])
                [pkgs.lighttpd]
                (pkgs.lib.attrValues packages)
              ;
            };
          }
      );
}
