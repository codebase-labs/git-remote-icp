# git-remote-icp

![](https://img.shields.io/badge/status%EF%B8%8F-experimental-blueviolet)
![](https://github.com/codebase-labs/git-remote-icp/actions/workflows/nix.yml/badge.svg?event=push)

A [Git remote helper](https://git-scm.com/docs/gitremote-helpers) for the [Internet Computer](https://internetcomputer.org) Protocol.

## Usage

1. Install to a location that is in your `PATH`.
2. Use `git` as you normally would, but use `icp://` instead of `https://` in URLs.


## Generating a public/private key pair

```
openssl ecparam -name secp256k1 -genkey -noout -out identity.pem
openssl ec -in secp256k1.pem -pubout -out public.pem
```

## Configuring Git

See the example `.gitconfig`

## Crates

This repository contains the following other crates:

* `git-remote-helper`

    A library for implementing Git remote helpers.

    Provides core functionality for remote helpers in a protocol-agnostic way for both blocking and async implementations.

* `git-remote-tcp`

    A Git remote helper for the `git://` protocol.

    Primarily used to test that the async implementation in `git-remote-helper` behaves the same as `git`.

* `git-remote-http-reqwest`

    A Git remote helper for `http://` and `https://` protocols.

    Primarily used to test that the blocking implementation in `git-remote-helper` behaves the same as `git` (`git-remote-http` and `git-remote-https`).

## Development

Set `HOME=.` when run from the root of this repository to use the provided `.gitconfig`.

### Against a local repository

```
cargo build --package git-remote-icp && PATH=./target/debug:$PATH RUST_LOG=trace HOME=. git clone icp::http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```

or

```
cargo build --package git-remote-icp && PATH=./target/debug:$PATH RUST_LOG=trace HOME=. git clone icp::http://git.codebase.ic0.localhost:8453/@paul/hello-world.git
```

### Against a remote repository

```
cargo build --package git-remote-icp && PATH=./target/debug:$PATH RUST_LOG=trace HOME=. git clone icp://w7uni-tiaaa-aaaam-qaydq-cai.raw.ic0.app/@paul/hello-world.git
```

### By manually invoking the remote helper

```
cargo build --package git-remote-icp && PATH=./target/debug:$PATH RUST_LOG=trace HOME=. GIT_DIR=~/temp/hello-world git-remote-icp origin http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```

or, without rebuilding:

```
RUST_LOG=trace HOME=. GIT_DIR=~/temp/hello-world cargo run origin http://rwlgt-iiaaa-aaaaa-aaaaa-cai.raw.ic0.localhost:8453/@paul/hello-world.git
```
