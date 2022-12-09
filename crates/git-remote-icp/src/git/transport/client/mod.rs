pub mod icp;
pub mod tcp;

use crate::cli::Cli;
use git::protocol::transport;
use git_repository as git;
use ic_agent::export::Principal;
use ic_agent::Identity;
use std::sync::Arc;
use transport::client::connect::Error;

pub async fn connect<'a, Url, E>(
    cli: &Cli,
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: &str,
    canister_id: Principal,
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send + 'a>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    match cli {
        Cli::GitRemoteIcp(_) => {
            icp::connect(identity, fetch_root_key, replica_url, canister_id, url, desired_version).await
        }
        Cli::GitRemoteTcp(_) => tcp::connect(url, desired_version).await,
    }
}
