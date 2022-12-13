{ pkgs, craneLib, cargoArtifacts, src }:

craneLib.buildPackage rec {
  pname = "git-remote-tcp";
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

    git clone git://localhost/.git test-repo-git
    git clone tcp://localhost/.git test-repo-tcp

    GIT_LOG_GIT=$(git -C test-repo-git log)
    GIT_LOG_TCP=$(git -C test-repo-tcp log)

    if [ "$GIT_LOG_GIT" == "$GIT_LOG_TCP" ]; then
      echo "GIT_LOG_GIT == GIT_LOG_TCP"
    else
      echo "GIT_LOG_GIT != GIT_LOG_TCP"
      exit 1
    fi

    GIT_DIFF_GIT=$(git -C test-repo-git diff)

    git -C test-repo-tcp remote add -f test-repo-git "$PWD/test-repo-git"
    git -C test-repo-tcp remote update
    GIT_DIFF_TCP=$(git -C test-repo-tcp diff main remotes/test-repo-git/main)

    if [ "$GIT_DIFF_GIT" == "$GIT_DIFF_TCP" ]; then
      echo "GIT_DIFF_GIT == GIT_DIFF_TCP"
    else
      echo "GIT_DIFF_GIT != GIT_DIFF_TCP"
      exit 1
    fi


    # Test push

    echo "\n" >> test-repo-git/README.md
    git -C test-repo-git add .
    git -C test-repo-git commit -m "Add trailing newline"
    git -C test-repo-git push origin main

    echo "\n" >> test-repo-tcp/README.md
    git -C test-repo-tcp add .
    git -C test-repo-tcp commit -m "Add trailing newline"
    git -C test-repo-tcp push origin main

    GIT_LOG_GIT_REMOTE=$(git -C test-repo-git log origin/main)
    GIT_LOG_TCP_REMOTE=$(git -C test-repo-tcp log origin/main)

    if [ "$GIT_LOG_GIT_REMOTE" == "$GIT_LOG_TCP_REMOTE" ]; then
      echo "GIT_LOG_GIT_REMOTE == GIT_LOG_TCP_REMOTE"
    else
      echo "GIT_LOG_GIT_REMOTE != GIT_LOG_TCP_REMOTE"
      echo "<<<<<<< GIT_LOG_GIT_REMOTE"
      echo "$GIT_LOG_GIT_REMOTE"
      echo "======="
      echo "$GIT_LOG_TCP_REMOTE"
      echo ">>>>>>> GIT_LOG_TCP_REMOTE"

      exit 1
    fi

    git -C test-repo-tcp remote update
    GIT_DIFF_GIT_REMOTE=$(git -C test-repo-git diff origin/main origin/main)
    GIT_DIFF_TCP_REMOTE=$(git -C test-repo-tcp diff origin/main remotes/test-repo-git/main)

    if [ "$GIT_DIFF_GIT_REMOTE" == "$GIT_DIFF_TCP_REMOTE" ]; then
      echo "GIT_DIFF_GIT_REMOTE == GIT_DIFF_TCP_REMOTE"
    else
      echo "GIT_DIFF_GIT_REMOTE != GIT_DIFF_TCP_REMOTE"
      echo "<<<<<<< GIT_DIFF_GIT_REMOTE"
      echo "$GIT_DIFF_GIT_REMOTE"
      echo "======="
      echo "$GIT_DIFF_TCP_REMOTE"
      echo ">>>>>>> GIT_DIFF_TCP_REMOTE"

      exit 1
    fi


    # Exit cleanly

    kill "$GIT_DAEMON_PID"
  '';
}
