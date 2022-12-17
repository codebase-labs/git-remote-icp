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

          lighttpd-conf-common = ''
            cgi.assign = ("" => "")

            setenv.set-environment = (
              "GIT_PROJECT_ROOT" => var.CWD,
              "GIT_HTTP_EXPORT_ALL" => "",
            )

            $REQUEST_HEADER["Git-Protocol"] =~ "(.+)" {
              setenv.add-environment += (
                "HTTP_GIT_PROTOCOL" => "%1",
              )
            }
          '';

          lighttpd-userfile = pkgs.writeText "userfile" ''
            username:password
          '';

          lighttpd-conf-auth = ''
            auth.backend = "plain"
            auth.backend.plain.userfile = "${lighttpd-userfile}"

            auth.require = (
              "/" => (
                "method" => "basic",
                "realm" => "git",
                "require" => "valid-user"
              )
            )
          '';

          # https://git-scm.com/docs/git-http-backend#Documentation/git-http-backend.txt-Lighttpd
          # https://github.com/NixOS/nixpkgs/blob/c7c950be8900e7ea5d2af4a5dfa58905ac612f84/nixos/modules/services/web-servers/lighttpd/default.nix
          lighttpd-conf = port: pkgs.writeText "lighthttpd.conf" ''
            server.document-root = var.CWD
            server.port = ${toString port}

            server.modules = (
              # "mod_auth",
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

            $HTTP["querystring"] =~ "service=git-receive-pack" {
              $HTTP["url"] =~ "^/git" {
                ${lighttpd-conf-common}
                # ${lighttpd-conf-auth}
              }
            } else $HTTP["url"] =~ "^/git/.*/git-receive-pack$" {
              ${lighttpd-conf-common}
              # ${lighttpd-conf-auth}
            } else $HTTP["url"] =~ "^/git" {
              ${lighttpd-conf-common}
            }
          '';

          git-remote-http-reqwest = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "http"; external = "http-reqwest"; };
            path_ = "/git";
            port = 8888;
            installCheckInputs = [
              pkgs.lighttpd
            ];
            configure = ''
              # git config --global credential.helper "!f() { echo \"username=username\"; echo \"password=password\"; }; f";
              git config --global --type bool http.receivePack true
            '';
            setup = ''
              pushd test-repo-bare
              lighttpd -f ${lighttpd-conf port} -D 2>&1 &
              HTTP_SERVER_PID=$!
              trap "EXIT_CODE=\$? && kill \$HTTP_SERVER_PID && exit \$EXIT_CODE" EXIT
              popd
            '';
            teardown = ''
              kill "$HTTP_SERVER_PID"
            '';
          };

          git-remote-icp = pkgs.callPackage ./nix/git-remote-helper.nix {
            inherit craneLib src;
            scheme = { internal = "http"; external = "icp"; };
            configure = ''
              git config --global --type bool icp.fetchRootKey true
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
            port = 9418;
            setup = ''
              # Start Git daemon

              # Based on https://github.com/Byron/gitoxide/blob/0c9c48b3b91a1396eb1796f288a2cb10380d1f14/tests/helpers.sh#L59
              git daemon --verbose --base-path=test-repo-bare --enable=receive-pack --export-all &
              GIT_DAEMON_PID=$!

              trap "EXIT_CODE=\$? && kill \$GIT_DAEMON_PID && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              kill "$GIT_DAEMON_PID"
            '';
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
              lighttpd-conf = lighttpd-conf 8888;
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
                [pkgs.lighttpd]
                (pkgs.lib.attrValues packages)
              ;
            };
          }
      );
}
