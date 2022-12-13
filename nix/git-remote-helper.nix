{ pkgs, craneLib, cargoArtifacts, src, scheme, setup, teardown }:

let SCHEME = {
  INTERNAL = pkgs.lib.toUpper scheme.internal;
  EXTERNAL = pkgs.lib.toUpper scheme.external;
}; in

craneLib.buildPackage {
  inherit cargoArtifacts src;
  pname = "git-remote-${scheme.external}";
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

    # git config --global icp.fetchRootKey true
    # git config --global icp.replicaUrl http://localhost:8000
    # git config --global icp.canisterId rwlgt-iiaaa-aaaaa-aaaaa-cai
    # git config --global icp.privateKey "$PWD/identity.pem"


    # Set up test repo

    mkdir test-repo
    git -C test-repo init
    echo "# Hello, World!" > test-repo/README.md
    git -C test-repo add .
    git -C test-repo commit -m "Initial commit"

    ${setup}

    # Test clone

    git clone ${scheme.internal}://localhost/.git test-repo-${scheme.internal}
    git clone ${scheme.external}://localhost/.git test-repo-${scheme.external}

    GIT_LOG_${SCHEME.INTERNAL}=$(git -C test-repo-${scheme.internal} log)
    GIT_LOG_${SCHEME.EXTERNAL}=$(git -C test-repo-${scheme.external} log)

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

    echo "\n" >> test-repo-${scheme.internal}/README.md
    git -C test-repo-${scheme.internal} add .
    git -C test-repo-${scheme.internal} commit -m "Add trailing newline"
    git -C test-repo-${scheme.internal} push origin main

    echo "\n" >> test-repo-${scheme.external}/README.md
    git -C test-repo-${scheme.external} add .
    git -C test-repo-${scheme.external} commit -m "Add trailing newline"
    git -C test-repo-${scheme.external} push origin main

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

    ${teardown}
  '';
}
