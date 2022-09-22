#![deny(rust_2018_idioms)]

use clap::Parser;
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

enum Commands {
    Capabiliites,
    List,
}

fn main() {
    let args = Args::parse();
    println!("args.repository: {:?}", args.repository);
    println!("args.url: {:?}", args.url);

    let url = match args.url.strip_prefix("ic::") {
        // Assume ic::<transport>://<address>
        Some(u) => Url::parse(&u).expect(format!("failed to parse URL: {:#?}", u).as_str()),
        // Assume ic://<address>
        None => {
            let u =
                Url::parse(&args.url).expect(format!("failed to parse URL: {}", args.url).as_str());

            // We want to change the scheme from "ic" to "https" but can't do
            // that with `u.set_scheme("https")` so need to create a new URL
            // that already has that scheme and change the other parts of the
            // URL instead.
            //
            // See https://github.com/servo/rust-url/pull/768
            let mut new_url = Url::parse("https://0.0.0.0")
                .expect(format!("failed to parse URL: {:#?}", args.url).as_str());

            new_url.set_fragment(u.fragment());

            let host = u.host_str();
            new_url.set_host(host).expect(
                format!("failed to set host: {:#?}, for URL: {:#?}", host, new_url).as_str(),
            );

            let password = u.password();
            new_url.set_password(password).expect(
                format!(
                    "failed to set password: {:#?}, for URL: {:#?}",
                    password, new_url
                )
                .as_str(),
            );

            new_url.set_path(u.path());

            let port = u.port();
            new_url.set_port(port).expect(
                format!("failed to set port: {:#?}, for URL: {:#?}", port, new_url).as_str(),
            );

            new_url.set_query(u.query());

            let username = u.username();
            new_url.set_username(username).expect(
                format!(
                    "failed to set username: {:#?}, for URL: {:#?}",
                    username, new_url
                )
                .as_str(),
            );

            new_url
        }
    };

    println!("url: {}", url);
}
