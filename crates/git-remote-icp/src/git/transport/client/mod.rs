pub mod icp;
pub mod tcp;

use ic_agent::Agent;
use crate::cli::Cli;
use git_repository as git;
use git::protocol::transport;
use transport::client::connect::Error;

pub async fn connect<Url, E>(
    cli: &Cli,
    agent: Agent,
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    match cli {
        Cli::GitRemoteIcp(_) => icp::connect(agent, url, desired_version).await,
        Cli::GitRemoteTcp(_) => tcp::connect(url, desired_version).await,
    }
}
