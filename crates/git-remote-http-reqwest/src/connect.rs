use git::protocol::transport;
use git::url::Scheme;
use git_repository as git;
use log::trace;
use std::convert::{Infallible, TryInto};
use transport::client::connect::Error;

pub fn connect<Url, E>(
    url: Url,
    options: transport::connect::Options,
) -> Result<Box<dyn transport::client::Transport + Send>, Error>
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    let mut url: git::Url = url.try_into().map_err(git::url::parse::Error::from)?;
    trace!("Provided URL scheme: {:#?}", url.scheme);

    url.scheme = match url.scheme {
        Scheme::Ext(scheme) if &scheme == "http-reqwest" => Ok(Scheme::Http),
        Scheme::Ext(scheme) if &scheme == "https-reqwest" => Ok(Scheme::Https),
        scheme @ (Scheme::Http | Scheme::Https) => Ok(scheme),
        _ => Err(Error::UnsupportedScheme(url.scheme)),
    }?;
    trace!("Resolved URL scheme: {:#?}", url.scheme);

    transport::connect::<_, Infallible>(url, options)
}
