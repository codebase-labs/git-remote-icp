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

          # https://git-scm.com/docs/git-http-backend#Documentation/git-http-backend.txt-Apache2x
          httpd-conf = port: pkgs.writeText "httpd.conf" ''
            ServerName localhost
            Listen ${toString port}

            LogLevel debug
            ErrorLog /dev/stdout

            SetEnv GIT_PROJECT_ROOT /
            SetEnv GIT_HTTP_EXPORT_ALL
            ScriptAlias /git/ ${pkgs.git}/libexec/git-core/git-http-backend/
            # SetEnvIf Git-Protocol ".*" GIT_PROTOCOL=$0

            LoadModule mpm_event_module ${pkgs.apacheHttpd}/modules/mod_mpm_event.so
            LoadModule cgi_module ${pkgs.apacheHttpd}/modules/mod_cgi.so
            LoadModule alias_module ${pkgs.apacheHttpd}/modules/mod_alias.so
            LoadModule env_module ${pkgs.apacheHttpd}/modules/mod_env.so
            LoadModule unixd_module ${pkgs.apacheHttpd}/modules/mod_unixd.so
          '';

          git-remote-http-reqwest = pkgs.callPackage ./nix/git-remote-helper.nix rec {
            inherit craneLib src;
            scheme = { internal = "http"; external = "http-reqwest"; };
            path_ = "/git/test-repo-bare";
            port = 8888;
            installCheckInputs = [
              pkgs.apacheHttpd
            ];
            configure = ''
              git config --global --type bool http.receivePack true
            '';
            setup = ''
              # Start HTTP server

              apachectl -k start -f ${httpd-conf port}

              trap "EXIT_CODE=\$? && apachectl -k stop && exit \$EXIT_CODE" EXIT
            '';
            teardown = ''
              apachectl -k stop
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
                [
                  pkgs.apacheHttpd
                ]
                (pkgs.lib.attrValues packages)
              ;
            };
          }
      );
}
