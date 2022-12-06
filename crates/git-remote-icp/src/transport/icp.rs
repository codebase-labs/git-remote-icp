use git_repository as git;
use git::protocol::transport;
use transport::client::connect::Error;

pub static SCHEME: &str = "icp";
pub static PROTOCOL: &str = "https";

pub async fn connect<Url, E>(
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    // FIXME
    Err(transport::client::connect::Error::Connection(anyhow::anyhow!("FIXME").into()))
}
