pub mod icp;
pub mod tcp;

use crate::cli::Cli;
use git::protocol::transport;
use git_repository as git;
use ic_agent::Identity;
use std::sync::Arc;
use transport::client::connect::Error;

pub async fn connect<'a, Url, E>(
    cli: &Cli,
    identity: Arc<dyn Identity>,
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send + 'a>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    match cli {
        Cli::GitRemoteIcp(_) => icp::connect(identity, url, desired_version).await,
        Cli::GitRemoteTcp(_) => tcp::connect(url, desired_version).await,
    }
}
