#![deny(rust_2018_idioms)]

use clap::{Command, FromArgMatches as _, Parser, Subcommand as _};
use url::Url;

#[derive(Parser)]
#[clap(about, version)]
struct Cli {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    repository: String,

    /// A URL of the form ic://<address> or ic::<transport>://<address>
    #[clap(value_parser)]
    url: String,
}

#[derive(Parser)]
enum Commands {
    Capabilities,
    List,
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    eprintln!("cli.repository: {:?}", cli.repository);
    eprintln!("cli.url: {:?}", cli.url);

    let url = match cli.url.strip_prefix("ic::") {
        // Assume ic::<transport>://<address>
        Some(u) => Url::parse(&u)
            .map_err(|error| format!("failed to parse URL: {:?}, with error: {:?}", u, error)),
        // Assume ic://<address>
        None => {
            let u = Url::parse(&cli.url).map_err(|error| {
                format!(
                    "failed to parse URL: {:?}, with error: {:?}",
                    cli.url, error
                )
            })?;

            // We want to change the scheme from "ic" to "https" but can't do
            // that with `u.set_scheme("https")` so need to create a new URL
            // that already has that scheme and change the other parts of the
            // URL instead.
            //
            // See https://github.com/servo/rust-url/pull/768
            let mut new_url = Url::parse("https://0.0.0.0")
                .map_err(|e| format!("failed to parse URL: {:?}, {}", cli.url, e))?;

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

    loop {
        eprintln!("loop");

        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("failed to read from stdin with error: {:?}", error))?;

        let input = input.trim();

        eprintln!("input: {:#?}", input);

        let input_command = Command::new("input")
            .help_template("")
            .multicall(true)
            .subcommand_required(true);

        let input_command = Commands::augment_subcommands(input_command);

        let matches = input_command
            .try_get_matches_from([input])
            .map_err(|e| e.to_string())?;

        let command = Commands::from_arg_matches(&matches).map_err(|e| e.to_string())?;

        match command {
            Commands::Capabilities => eprintln!("got capabilities"),
            Commands::List => eprintln!("got list"),
        }
    }
}
