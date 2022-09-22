#![deny(rust_2018_idioms)]

use clap::{Parser, ValueEnum};
use url::Url;

#[derive(Parser)]
#[clap(about, version)]
struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    repository: String,

    /// A URL of the form ic://<address> or ic::<transport>://<address>
    #[clap(value_parser)]
    url: String,
}

#[derive(Copy, Clone, ValueEnum)]
enum Commands {
    Capabiliites,
    List,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    eprintln!("args.repository: {:?}", args.repository);
    eprintln!("args.url: {:?}", args.url);

    let url = match args.url.strip_prefix("ic::") {
        // Assume ic::<transport>://<address>
        Some(u) => Url::parse(&u)
            .map_err(|error| format!("failed to parse URL: {:?}, with error: {:?}", u, error)),
        // Assume ic://<address>
        None => {
            let u = Url::parse(&args.url).map_err(|error| {
                format!(
                    "failed to parse URL: {:?}, with error: {:?}",
                    args.url, error
                )
            })?;

            // We want to change the scheme from "ic" to "https" but can't do
            // that with `u.set_scheme("https")` so need to create a new URL
            // that already has that scheme and change the other parts of the
            // URL instead.
            //
            // See https://github.com/servo/rust-url/pull/768
            let mut new_url = Url::parse("https://0.0.0.0")
                .map_err(|e| format!("failed to parse URL: {:?}, {}", args.url, e))?;

            new_url.set_fragment(u.fragment());

            let host = u.host_str();
            new_url.set_host(host).map_err(|error| {
                format!(
                    "failed to set host: {:?}, for URL: {:?}, with error: {}",
                    host, new_url, error
                )
            })?;

            let password = u.password();
            new_url.set_password(password).map_err(|error| {
                format!(
                    "failed to set password: {:?}, for URL: {:?}, with error: {:?}",
                    password, new_url, error
                )
            })?;

            new_url.set_path(u.path());

            let port = u.port();
            new_url.set_port(port).map_err(|error| {
                format!(
                    "failed to set port: {:?}, for URL: {:?}, with error: {:?}",
                    port, new_url, error
                )
            })?;

            new_url.set_query(u.query());

            let username = u.username();
            new_url.set_username(username).map_err(|error| {
                format!(
                    "failed to set username: {:?}, for URL: {:?}, with error: {:?}",
                    username, new_url, error
                )
            })?;

            Ok(new_url)
        }
    }?;

    eprintln!("url: {}", url);

    Ok(())
}
