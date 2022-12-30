use crate::http::Remote;

use git::protocol::transport;
use git::url::Scheme;
use git_repository as git;
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::export::Principal;
use ic_agent::{Agent, Identity};
use log::trace;
use std::sync::Arc;
use tokio::runtime::Runtime;
use transport::client::connect::Error;

pub fn connect<'a, Url, E>(
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: String,
    canister_id: Principal,
) -> impl Fn(Url, transport::Protocol) -> Result<Box<dyn transport::client::Transport + Send + 'a>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    trace!("identity: {:#?}", identity);
    trace!("fetch_root_key: {:#?}", fetch_root_key);
    trace!("replica_url: {}", replica_url);
    trace!("canister_id: {}", canister_id);

    move |url: Url, desired_version| {
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

        let replica_transport = ReqwestHttpReplicaV2Transport::create(&replica_url)
            .map_err(|err| Error::Connection(Box::new(err)))?;

        let agent = Agent::builder()
            .with_transport(replica_transport)
            .with_arc_identity(identity.clone())
            .build()
            .map_err(|err| Error::Connection(Box::new(err)))?;

        if fetch_root_key {
            let runtime = Runtime::new().map_err(|err| Error::Connection(Box::new(err)))?;

            runtime
                .block_on(agent.fetch_root_key())
                .map_err(|err| Error::Connection(Box::new(err)))?;
        }

        let remote = Remote::new(agent, canister_id);

        let transport = transport::client::http::connect_http(
            remote,
            &url.to_bstring().to_string(),
            desired_version,
        );

        Ok(Box::new(transport))
    }
}
