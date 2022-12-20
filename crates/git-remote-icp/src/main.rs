#![feature(async_closure)]
#![feature(impl_trait_in_fn_trait_return)]

mod async_io;
mod config;
mod connect;
mod connection;

use anyhow::anyhow;
use git_remote_helper;
use log::trace;
use std::sync::Arc;
use ic_agent::identity::{Identity, Secp256k1Identity};

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let private_key_path = config::private_key()?;
    trace!("private key path: {}", private_key_path);

    let identity = Secp256k1Identity::from_pem_file(private_key_path)?;
    let identity = Arc::new(identity);

    let principal = identity.sender().map_err(|err| anyhow!(err))?;
    trace!("principal: {}", principal);

    let fetch_root_key = config::fetch_root_key();
    trace!("fetch root key: {}", fetch_root_key);

    let replica_url = config::replica_url();
    trace!("replica url: {}", replica_url);

    let canister_id = config::canister_id()?;
    trace!("canister id: {}", canister_id);

    let connect = connect::make::<String, _>(identity, fetch_root_key, &replica_url, canister_id);

    git_remote_helper::main(connect).await
}
