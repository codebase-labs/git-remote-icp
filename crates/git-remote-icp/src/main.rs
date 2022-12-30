mod config;
mod connect;
mod http;

use anyhow::anyhow;
use git_remote_helper;
use ic_agent::identity::{AnonymousIdentity, Identity, Secp256k1Identity};
use log::trace;
use std::sync::Arc;

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    let private_key_path = config::private_key();
    trace!("private key path: {:#?}", private_key_path);

    let identity = get_identity(private_key_path)?;

    let principal = identity.sender().map_err(|err| anyhow!(err))?;
    trace!("principal: {}", principal);
    eprintln!("Principal for caller: {}", principal);

    let fetch_root_key = config::fetch_root_key();
    trace!("fetch root key: {}", fetch_root_key);

    let replica_url = config::replica_url();
    trace!("replica url: {}", replica_url);

    let canister_id = config::canister_id()?;
    trace!("canister id: {}", canister_id);

    git_remote_helper::main(connect::connect(
        identity,
        fetch_root_key,
        replica_url,
        canister_id,
    ))
}

fn get_identity(private_key_path: anyhow::Result<String>) -> anyhow::Result<Arc<dyn Identity>> {
    match private_key_path {
        Ok(path) => {
            eprintln!("Using identity for private key found in git config");
            let identity = Secp256k1Identity::from_pem_file(path)?;
            Ok(Arc::new(identity))
        }
        Err(_) => {
            eprintln!("No private key found git config, using anonymous identity");
            Ok(Arc::new(AnonymousIdentity {}))
        }
    }
}
