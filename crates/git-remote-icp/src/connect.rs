// use crate::connection::Connection;
use crate::http::Remote;

use git::protocol::transport;
use git::url::Scheme;
use git_repository as git;
use ic_agent::export::Principal;
use ic_agent::identity::Identity;
use log::trace;
use std::convert::Infallible;
use std::sync::Arc;
use transport::client::connect::Error;

pub fn make<Url, E>(
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: &str,
    canister_id: Principal,
) -> impl Fn(Url, transport::Protocol) -> Result<Box<dyn transport::client::Transport + Send>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    // TODO:
    // * `transport::client::connect` (`transport::client::blocking_io::http::connect`) returns:
    // * `Transport<Impl>` (`transport::client::blocking_io::http::Transport`)
    // * where `Impl` is `H: Http`
    // * where `pub type Impl = reqwest::Remote` (or `curl::Curl`)
    // * `reqwest::Remote` is `transport::client::http::reqwest::Remote`
    // * `impl Default for Remote` is in transport/blocking_io/http/reqwest/remote
    |url: Url, desired_version| {
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

        connect::<_, Infallible>(url, desired_version)
    }
}

// TEMP: we likely need to inline this at the end of `make`
pub fn connect<Url, E>(
    url: Url,
    desired_version: transport::Protocol,
) -> Result<Box<dyn transport::client::Transport + Send>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    let mut url: git::Url = url.try_into().map_err(git::url::parse::Error::from)?;
    trace!("Provided URL scheme: {:#?}", url.scheme);

    url.scheme = match url.scheme {
        Scheme::Ext(scheme) if &scheme == "icp" => Ok(Scheme::Https),
        scheme @ (Scheme::Http | Scheme::Https) => Ok(scheme),
        _ => Err(Error::UnsupportedScheme(url.scheme)),
    }?;
    trace!("Resolved URL scheme: {:#?}", url.scheme);

    // TODO: replace the placeholder generic argument with our `Remote` that
    // implements the `Http` trait.
    let remote: Remote = todo!();

    let transport = transport::client::http::connect_http(
        remote,
        &url.to_bstring().to_string(),
        desired_version,
    );

    Ok(Box::new(transport))
}
