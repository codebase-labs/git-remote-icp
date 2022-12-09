#![deny(rust_2018_idioms)]

mod cli;
mod commands;
mod git;

use anyhow::{anyhow, Context};
use clap::{Command, FromArgMatches as _, Parser, Subcommand as _};
use cli::Cli;
use commands::Commands;
use git_repository as gitoxide;
use ic_agent::identity::{Identity as _, Secp256k1Identity};
use log::trace;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::sync::Arc;
use strum::VariantNames as _;

const GIT_DIR: &str = "GIT_DIR";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let args = cli::args(&cli);
    trace!("args.repository: {:?}", args.repository);
    trace!("args.url: {:?}", args.url);

    gitoxide::interrupt::init_handler(move || {})?;

    let git_dir = env::var(GIT_DIR).context("failed to get GIT_DIR")?;
    trace!("GIT_DIR: {}", git_dir);

    let repo_dir = Path::new(&git_dir)
        .parent()
        .ok_or_else(|| anyhow!("failed to get repository directory"))?;

    let repo = gitoxide::open(repo_dir)?;

    // TODO: consider falling back to AnonymousIdentity if icp.privateKey isn't
    // set to allow users to clone from public repos using the icp:// scheme.
    let private_key_path = git::config::private_key().map_err(|_| {
        anyhow!("failed to read icp.privateKey from git config. Set with `git config --global icp.privateKey <path to private key>`")
    })?;

    trace!("private key path: {}", private_key_path);

    let identity = Secp256k1Identity::from_pem_file(private_key_path)?;
    let identity = Arc::new(identity);

    let principal = identity.sender().map_err(|err| anyhow!(err))?;
    trace!("principal: {}", principal);

    let fetch_root_key = git::config::fetch_root_key();
    trace!("fetch_root_key: {}", fetch_root_key);

    let replica_url = git::config::replica_url();
    trace!("replica_url: {}", replica_url);

    let canister_id = git::config::canister_id()?;
    trace!("canister_id: {}", canister_id);

    let authenticate =
        |action| panic!("unexpected call to authenticate with action: {:#?}", action);

    let mut fetch: commands::fetch::Batch = BTreeSet::new();
    let mut push: commands::push::Batch = BTreeSet::new();

    loop {
        trace!("loop");

        // TODO: BString?
        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read from stdin")?;

        let input = input.trim();

        if input.is_empty() {
            trace!("terminated with a blank line");

            let fetch_transport = git::transport::client::connect(
                &cli,
                identity.clone(),
                fetch_root_key,
                replica_url.as_str(),
                canister_id.clone(),
                args.url.clone(),
                gitoxide::protocol::transport::Protocol::V2,
            )
            .await?;

            commands::fetch::process(fetch_transport, &repo, &args.url, &mut fetch).await?;

            // NOTE: push still uses the v1 protocol so we use that here.
            let mut push_transport = git::transport::client::connect(
                &cli,
                identity.clone(),
                fetch_root_key,
                replica_url.as_str(),
                canister_id.clone(),
                args.url.clone(),
                gitoxide::protocol::transport::Protocol::V1,
            )
            .await?;

            commands::push::process(&mut push_transport, &repo, authenticate, &mut push).await?;

            // continue; // Useful to inspect .git directory before it disappears
            break Ok(());
        }

        let input = input.split(' ').collect::<Vec<_>>();

        trace!("input: {:#?}", input);

        let input_command = Command::new("git-remote-icp")
            .multicall(true)
            .subcommand_required(true);

        let input_command = Commands::augment_subcommands(input_command);
        let matches = input_command.try_get_matches_from(input)?;
        let command = Commands::from_arg_matches(&matches)?;

        match command {
            Commands::Capabilities => {
                // TODO: buffer and flush
                Commands::VARIANTS
                    .iter()
                    .filter(|command| **command != "capabilities" && **command != "list")
                    .for_each(|command| println!("{}", command));
                println!();
            }
            Commands::Fetch { hash, name } => {
                trace!("batch fetch {} {}", hash, name);
                let _ = fetch.insert((hash, name));
            }
            Commands::List { variant } => {
                let mut transport = git::transport::client::connect(
                    &cli,
                    identity.clone(),
                    fetch_root_key,
                    replica_url.as_str(),
                    canister_id.clone(),
                    args.url.clone(),
                    gitoxide::protocol::transport::Protocol::V2,
                )
                .await?;

                commands::list::execute(&mut transport, authenticate, &variant).await?
            }
            Commands::Push { src_dst } => {
                trace!("batch push {}", src_dst);
                let _ = push.insert(src_dst);
            }
        }
    }
}
