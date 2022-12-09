# git-remote-icp

![](https://img.shields.io/badge/status%EF%B8%8F-experimental-blueviolet)

A Git remote helper for the Internet Computer Protocol.

## Usage

1. Install to a location that is in your `PATH`.
2. Use `git` as you normally would, but use `icp://` instead of `https://` in URLs.


## Generating a public/private key pair

```
openssl ecparam -name secp256k1 -genkey -noout -out identity.pem
openssl ec -in secp256k1.pem -pubout -out public.pem
```

## Configuring Git

### Globally

```
git config --global icp.privateKey /absolute/path/to/private/key/file
```

## Resources

* https://git-scm.com/docs/gitremote-helpers
* https://rovaughn.github.io/2015-2-9.html

## Debugging

### Against a local repository

```
cargo build && PATH=./target/debug:$PATH RUST_LOG=trace git -c icp.fetchRootKey=true -c icp.replicaUrl=http://localhost:8000 -c icp.canisterId=rwlgt-iiaaa-aaaaa-aaaaa-cai -c icp.privateKey=/Users/py/projects/codebase-labs/git-remote-icp/main/identity.pem clone icp::http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```

or

```
cargo build && PATH=./target/debug:$PATH RUST_LOG=trace git -c icp.fetchRootKey=true -c icp.replicaUrl=http://localhost:8000 -c icp.canisterId=rwlgt-iiaaa-aaaaa-aaaaa-cai -c icp.privateKey=/Users/py/projects/codebase-labs/git-remote-icp/main/identity.pem clone icp::http://git.codebase.ic0.localhost:8453/@paul/hello-world.git
```

### Against a remote repository

```
cargo build && PATH=./target/debug:$PATH RUST_LOG=trace git clone icp://w7uni-tiaaa-aaaam-qaydq-cai.raw.ic0.app/@paul/hello-world.git
```

### By manually invoking the remote helper

```
cargo build && PATH=./target/debug:$PATH RUST_LOG=trace GIT_DIR=~/temp/hello-world git-remote-icp origin http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```

or, without rebuilding:

```
RUST_LOG=trace GIT_DIR=~/temp/hello-world cargo run origin http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```
