#![feature(async_closure)]
#![feature(impl_trait_in_fn_trait_return)]

mod config;

use anyhow::{anyhow, Context};
use git::protocol::transport;
use git_remote_helper;
use ic_agent::export::Principal;
use git_repository as git;
use log::trace;
use std::sync::Arc;
use transport::client::connect::Error;
use ic_agent::identity::{Identity, Secp256k1Identity};
use std::future::Future;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // TODO: consider falling back to AnonymousIdentity if icp.privateKey isn't
    // set to allow users to clone from public repos using the icp:// scheme.
    let private_key_path = config::private_key().map_err(|_| {
        anyhow!("failed to read icp.privateKey from git config. Set with `git config --global icp.privateKey <path to private key>`")
    })?;

    trace!("private key path: {}", private_key_path);

    let identity = Secp256k1Identity::from_pem_file(private_key_path)?;
    let identity = Arc::new(identity);

    let principal = identity.sender().map_err(|err| anyhow!(err))?;
    trace!("principal: {}", principal);

    let fetch_root_key = config::fetch_root_key();
    trace!("fetch_root_key: {}", fetch_root_key);

    let replica_url = config::replica_url();
    trace!("replica_url: {}", replica_url);

    let canister_id = config::canister_id()?;
    trace!("canister_id: {}", canister_id);

    let connect = make_connect::<String, _>(identity, fetch_root_key, &replica_url, canister_id);

    git_remote_helper::main(connect).await
}

fn make_connect<'a, Url, E>(
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: &str,
    canister_id: Principal,
) -> impl Fn(Url, transport::Protocol) -> (impl Future<Output = Result<Box<(dyn transport::client::Transport + Send + 'a)>, transport::connect::Error>> + 'a)
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    let connect = async move |url, desired_version| {
      todo!()
    };
    connect
}
