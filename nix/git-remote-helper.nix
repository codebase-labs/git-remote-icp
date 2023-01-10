{ pkgs
, craneLib
, src
, scheme
, port
, path_ ? "/"
, installCheckInputs ? []
, doInstallCheck ? true
, configure ? ""
, setup
, teardown
}:

let
  schemeToEnvVar = scheme:
    builtins.replaceStrings ["-"] ["_"] (pkgs.lib.toUpper scheme);

  SCHEME = {
    INTERNAL = schemeToEnvVar scheme.internal;
    EXTERNAL = schemeToEnvVar scheme.external;
  };

  pname = "git-remote-${scheme.external}";
  cargoExtraArgs = "--package ${pname}";
in
  craneLib.buildPackage {
    inherit cargoExtraArgs pname src doInstallCheck;
    cargoArtifacts = craneLib.buildDepsOnly {
      inherit cargoExtraArgs pname src;
    };
    nativeBuildInputs = [
      pkgs.darwin.apple_sdk.frameworks.Security
    ];
    installCheckInputs = installCheckInputs ++ [
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
      git config --global protocol.version 2

      echo "---------------"
      echo "configure start"
      echo "---------------"

      ${configure}

      echo "------------------"
      echo "configure complete"
      echo "------------------"

      # Set up test repo

      mkdir test-repo
      git -C test-repo init
      echo '# Hello, World!' > test-repo/README.md
      git -C test-repo add .
      git -C test-repo commit -m "Initial commit"

      GIT_LOG_INIT=$(git -C test-repo log)

      git clone --bare test-repo test-repo-bare
      git -C test-repo-bare update-server-info

      echo "-----------"
      echo "setup start"
      echo "-----------"

      ${setup}

      echo "--------------"
      echo "setup complete"
      echo "--------------"

      while ! nc -z localhost ${toString port}; do
        sleep 0.1
      done


      # Test clone

      echo "------------------"
      echo "native clone start"
      echo "------------------"

      git clone ${scheme.internal}://localhost:${toString port}${path_} test-repo-${scheme.internal}

      echo "---------------------"
      echo "native clone complete"
      echo "---------------------"

      echo "-------------------------"
      echo "remote helper clone start"
      echo "-------------------------"

      git clone ${scheme.external}://localhost:${toString port}${path_} test-repo-${scheme.external}

      echo "----------------------------"
      echo "remote helper clone complete"
      echo "----------------------------"

      GIT_LOG_${SCHEME.INTERNAL}=$(git -C test-repo-${scheme.internal} log)
      GIT_LOG_${SCHEME.EXTERNAL}=$(git -C test-repo-${scheme.external} log)

      if [ "$GIT_LOG_INIT" == "$GIT_LOG_${SCHEME.INTERNAL}" ]; then
        echo "GIT_LOG_INIT == GIT_LOG_${SCHEME.INTERNAL}"
      else
        echo "GIT_LOG_INIT != GIT_LOG_${SCHEME.INTERNAL}"
        exit 1
      fi

      if [ "$GIT_LOG_${SCHEME.INTERNAL}" == "$GIT_LOG_${SCHEME.EXTERNAL}" ]; then
        echo "GIT_LOG_${SCHEME.INTERNAL} == GIT_LOG_${SCHEME.EXTERNAL}"
      else
        echo "GIT_LOG_${SCHEME.INTERNAL} != GIT_LOG_${SCHEME.EXTERNAL}"
        exit 1
      fi

      GIT_DIFF_${SCHEME.INTERNAL}=$(git -C test-repo-${scheme.internal} diff)

      git -C test-repo-${scheme.external} remote add -f test-repo-${scheme.internal} "$PWD/test-repo-${scheme.internal}"
      git -C test-repo-${scheme.external} remote update
      GIT_DIFF_${SCHEME.EXTERNAL}=$(git -C test-repo-${scheme.external} diff main remotes/test-repo-${scheme.internal}/main)

      if [ "$GIT_DIFF_${SCHEME.INTERNAL}" == "$GIT_DIFF_${SCHEME.EXTERNAL}" ]; then
        echo "GIT_DIFF_${SCHEME.INTERNAL} == GIT_DIFF_${SCHEME.EXTERNAL}"
      else
        echo "GIT_DIFF_${SCHEME.INTERNAL} != GIT_DIFF_${SCHEME.EXTERNAL}"
        exit 1
      fi


      # Test push

      echo "-----------------"
      echo "native push start"
      echo "-----------------"

      echo "" >> test-repo-${scheme.internal}/README.md
      git -C test-repo-${scheme.internal} add .
      git -C test-repo-${scheme.internal} commit -m "Add trailing newline"
      git -C test-repo-${scheme.internal} push origin main

      echo "--------------------"
      echo "native push complete"
      echo "--------------------"

      echo "------------------------"
      echo "remote helper push start"
      echo "------------------------"

      echo "" >> test-repo-${scheme.external}/README.md
      git -C test-repo-${scheme.external} add .
      git -C test-repo-${scheme.external} commit -m "Add trailing newline"
      git -C test-repo-${scheme.external} push origin main

      echo "---------------------------"
      echo "remote helper push complete"
      echo "---------------------------"

      GIT_LOG_${SCHEME.INTERNAL}_REMOTE=$(git -C test-repo-${scheme.internal} log origin/main)
      GIT_LOG_${SCHEME.EXTERNAL}_REMOTE=$(git -C test-repo-${scheme.external} log origin/main)

      if [ "$GIT_LOG_${SCHEME.INTERNAL}_REMOTE" == "$GIT_LOG_${SCHEME.EXTERNAL}_REMOTE" ]; then
        echo "GIT_LOG_${SCHEME.INTERNAL}_REMOTE == GIT_LOG_${SCHEME.EXTERNAL}_REMOTE"
      else
        echo "GIT_LOG_${SCHEME.INTERNAL}_REMOTE != GIT_LOG_${SCHEME.EXTERNAL}_REMOTE"
        echo "<<<<<<< GIT_LOG_${SCHEME.INTERNAL}_REMOTE"
        echo "$GIT_LOG_${SCHEME.INTERNAL}_REMOTE"
        echo "======="
        echo "$GIT_LOG_${SCHEME.EXTERNAL}_REMOTE"
        echo ">>>>>>> GIT_LOG_${SCHEME.EXTERNAL}_REMOTE"

        exit 1
      fi

      git -C test-repo-${scheme.external} remote update
      GIT_DIFF_${SCHEME.INTERNAL}_REMOTE=$(git -C test-repo-${scheme.internal} diff origin/main origin/main)
      GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE=$(git -C test-repo-${scheme.external} diff origin/main remotes/test-repo-${scheme.internal}/main)

      if [ "$GIT_DIFF_${SCHEME.INTERNAL}_REMOTE" == "$GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE" ]; then
        echo "GIT_DIFF_${SCHEME.INTERNAL}_REMOTE == GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE"
      else
        echo "GIT_DIFF_${SCHEME.INTERNAL}_REMOTE != GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE"
        echo "<<<<<<< GIT_DIFF_${SCHEME.INTERNAL}_REMOTE"
        echo "$GIT_DIFF_${SCHEME.INTERNAL}_REMOTE"
        echo "======="
        echo "$GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE"
        echo ">>>>>>> GIT_DIFF_${SCHEME.EXTERNAL}_REMOTE"

        exit 1
      fi

      echo "--------------"
      echo "teardown start"
      echo "--------------"

      ${teardown}

      echo "-----------------"
      echo "teardown complete"
      echo "-----------------"
    '';
  }
