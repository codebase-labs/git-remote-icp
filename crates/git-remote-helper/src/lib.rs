#![deny(rust_2018_idioms)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_fn_trait_return)]

pub mod cli;
pub mod commands;
pub mod git;

use anyhow::{anyhow, Context};
use clap::{Command, FromArgMatches as _, Parser as _, Subcommand as _};
use cli::Args;
use commands::Commands;
use git_repository as gitoxide;
use gitoxide::protocol::transport;
use log::trace;
use maybe_async::maybe_async;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use strum::VariantNames as _;

#[cfg(all(feature = "async-network-client", feature = "blocking-network-client"))]
compile_error!("Cannot set both 'async-network-client' and 'blocking-network-client' features as they are mutually exclusive");

const GIT_DIR: &str = "GIT_DIR";

#[maybe_async]
pub async fn main<C>(connect: impl Fn(String, transport::Protocol) -> C) -> anyhow::Result<()>
where
    C: std::future::Future<
        Output = Result<
            Box<(dyn transport::client::Transport + Send)>,
            transport::client::connect::Error,
        >,
    >,
{
    let args = Args::parse();
    trace!("args.repository: {:?}", args.repository);
    trace!("args.url: {:?}", args.url);

    gitoxide::interrupt::init_handler(move || {})?;

    let git_dir = env::var(GIT_DIR).context("failed to get GIT_DIR")?;
    trace!("GIT_DIR: {}", git_dir);

    let repo_dir = Path::new(&git_dir)
        .parent()
        .ok_or_else(|| anyhow!("failed to get repository directory"))?;

    let repo = gitoxide::open(repo_dir)?;

    // TODO: implementer provides this
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

            let fetch_transport = connect(
                args.url.clone(),
                gitoxide::protocol::transport::Protocol::V2,
            )
            .await?;

            commands::fetch::process(fetch_transport, &repo, &args.url, &mut fetch).await?;

            // NOTE: push still uses the v1 protocol so we use that here.
            let mut push_transport = connect(
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
                let mut transport = connect(
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
