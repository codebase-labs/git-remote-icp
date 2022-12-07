use git::protocol::transport;
use git::url::Scheme;
use git_repository as git;
use log::trace;
use std::convert::{Infallible, TryInto};
use transport::client::connect::Error;

pub async fn connect<Url, E>(
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
        Scheme::Ext(scheme) if &scheme == "tcp" => Ok(Scheme::Git),
        scheme @ Scheme::Git => Ok(scheme),
        _ => Err(Error::UnsupportedScheme(url.scheme)),
    }?;
    trace!("Resolved URL scheme: {:#?}", url.scheme);

    transport::connect::<_, Infallible>(url, desired_version).await
}
