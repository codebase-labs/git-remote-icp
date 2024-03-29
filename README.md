# git-remote-icp

<picture><img src="https://img.shields.io/badge/status%EF%B8%8F-experimental-blueviolet"></picture>
[![](https://github.com/codebase-labs/git-remote-icp/actions/workflows/ci.yml/badge.svg?event=push)](https://github.com/codebase-labs/git-remote-icp/actions/workflows/ci.yml)

A [Git remote helper](https://git-scm.com/docs/gitremote-helpers) for the [Internet Computer](https://internetcomputer.org) Protocol.

## Demos

* Cloning a repo from [codebase.org](https://codebase.org), hosted on the Internet Computer, using the IC’s native auth:
  * [with the Git CLI](https://twitter.com/py/status/1608749309427879936)
  * [with GitHub Desktop](https://twitter.com/py/status/1608749699980464129)

## Usage

1. Install to a location that is in your `PATH`.
2. Use `git` as you normally would, but use `icp://` instead of `https://` in URLs.


## Generating a public/private key pair

```
openssl ecparam -name secp256k1 -genkey -noout -out identity.pem
openssl ec -in identity.pem -pubout -out identity.pub
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

* Set `HOME=.` when run from the root of this repository to use the provided `.gitconfig`.
* The `icp://` scheme requires HTTPS. Use `icp::http://` for local development.

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
