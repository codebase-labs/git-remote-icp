use crate::git::transport::client::icp;
use git::protocol::transport;
use git::url::Scheme;
use git_repository as git;
use ic_agent::export::Principal;
use ic_agent::Identity;
use log::trace;
use std::sync::Arc;
use transport::client::connect::Error;

pub async fn connect<'a, Url, E>(
    identity: Arc<dyn Identity>,
    replica_url: &str,
    canister_id: Principal,
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send + 'a>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    let mut url = url.try_into().map_err(git::url::parse::Error::from)?;

    if url.user().is_some() {
        return Err(Error::UnsupportedUrlTokens {
            url: url.to_bstring(),
            scheme: url.scheme,
        });
    }

    trace!("Provided URL scheme: {:#?}", url.scheme);

    url.scheme = match url.scheme {
        Scheme::Ext(scheme) if &scheme == "icp" => Ok(Scheme::Https),
        scheme @ (Scheme::Https | Scheme::Http) => Ok(scheme),
        _ => Err(Error::UnsupportedScheme(url.scheme)),
    }?;

    trace!("Resolved URL scheme: {:#?}", url.scheme);

    let connection =
        icp::Connection::new(identity, replica_url, canister_id, url, desired_version)?;

    Ok(Box::new(connection))
}
