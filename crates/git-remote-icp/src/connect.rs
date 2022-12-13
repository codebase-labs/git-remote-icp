use crate::connection::Connection;

use git::protocol::transport;
use ic_agent::export::Principal;
use ic_agent::identity::Identity;
use git_repository as git;
use git::url::Scheme;
use log::trace;
use std::future::Future;
use std::sync::Arc;
use transport::client::connect::Error;

pub fn make<'a, Url, E>(
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: &'a str,
    canister_id: Principal,
// ) -> impl Fn(Url, transport::Protocol) -> (impl Future<Output = Result<Box<(dyn transport::client::Transport + Send + 'a)>, transport::connect::Error>> + 'a)
) -> Box<dyn Fn(Url, transport::Protocol) -> (impl Future<Output = Result<Connection, transport::connect::Error>> + 'a) + 'a>
where
    Url: TryInto<git::url::Url, Error = E> + 'a,
    git::url::parse::Error: From<E>,
{
    let connect = async move |url: Url, desired_version| {
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
            Connection::new(identity, fetch_root_key, replica_url, canister_id, url, desired_version).await?;

        Ok(connection)
    };
    Box::new(connect)
}
