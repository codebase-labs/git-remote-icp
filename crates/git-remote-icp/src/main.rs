#![deny(rust_2018_idioms)]

mod commands;
mod git;

use git::service::receive_pack;
use anyhow::{anyhow, Context};
use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum};
use gitoxide::bstr::ByteSlice as _;
use git_repository as gitoxide;
use log::trace;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use strum::{EnumVariantNames, VariantNames as _};

#[derive(Debug, Parser)]
#[clap(multicall(true))]
#[clap(about, version)]
enum RemoteHelper {
    #[clap(name = "git-remote-icp")]
    ICP(Args),
    #[clap(name = "git-remote-tcp")]
    TCP(Args),
}

#[derive(Debug, Parser)]
#[clap(about, version)]
struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    repository: String,

    /// A URL of the form icp://<address> or icp::<transport>://<address>
    #[clap(value_parser)]
    url: String,
}

#[derive(Debug, EnumVariantNames, Eq, Ord, PartialEq, PartialOrd, Parser)]
#[strum(serialize_all = "kebab_case")]
enum Commands {
    Capabilities,
    Fetch {
        #[clap(value_parser)]
        hash: String, // TODO: gitoxide::hash::ObjectId?

        #[clap(value_parser)]
        name: String,
    },
    List {
        #[clap(arg_enum, value_parser)]
        variant: Option<ListVariant>,
    },
    Push {
        #[clap(value_parser)]
        src_dst: String,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
enum ListVariant {
    ForPush,
}

const GIT_DIR: &str = "GIT_DIR";

// TODO: modules for each command
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let remote_helper = RemoteHelper::parse();

    let (external_transport_protocol, internal_transport_protocol, args) = match remote_helper {
        RemoteHelper::ICP(args) => ("icp", "https", args),
        RemoteHelper::TCP(args) => ("tcp", "git", args),
    };

    trace!("external_transport_protocol: {:?}", external_transport_protocol);
    trace!("args.repository: {:?}", args.repository);
    trace!("args.url: {:?}", args.url);

    gitoxide::interrupt::init_handler(move || {})?;

    let git_dir = env::var(GIT_DIR).context("failed to get GIT_DIR")?;
    trace!("GIT_DIR: {}", git_dir);

    // TODO: use gitoxide here instead
    // `repo.config_snapshot().string(“icp.privateKey”)`
    let private_key_path = std::process::Command::new("git")
        .arg("config")
        .arg("icp.privateKey")
        .output()?;

    let private_key_path = private_key_path.stdout;
    let private_key_path = String::from_utf8(private_key_path)?;
    let private_key_path = private_key_path.trim();

    // if private_key_path.is_empty() {
    //     return Err(anyhow!("failed to read icp.privateKey from git config. Set with `git config --global icp.privateKey <path to private key>`"));
    // }

    trace!("private key path: {}", private_key_path);

    // let private_key_data = std::fs::read(private_key_path)
    //     .map_err(|err| anyhow!("failed to read private key: {}", err))?;

    // let private_key = _;

    // TODO: read icp.keyId from git config

    let url: String = match args.url.strip_prefix(&format!("{}://", external_transport_protocol)) {
        // The supplied URL was of the form `icp://<address>` so we change it to
        // `https://<address>`.
        Some(address) => format!("{}://{}", internal_transport_protocol, address),
        // The supplied url was of the form `icp::<transport>://<address>` but
        // Git invoked the remote helper with `<transport>://<address>`
        None => args.url.to_string(),
    };

    trace!("url: {}", url);

    let repo_dir = Path::new(&git_dir)
        .parent()
        .ok_or_else(|| anyhow!("failed to get repository directory"))?;

    // TODO: `repo.config_snapshot().string(“icp.privateKey”)`
    let repo = gitoxide::open(repo_dir)?;

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
            commands::fetch::process(&repo, &url, &mut fetch).await?;
            commands::push::process(&repo, &url, authenticate, &mut push).await?;
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
                match variant {
                    Some(x) => match x {
                        ListVariant::ForPush => trace!("list for-push"),
                    },
                    None => {
                        trace!("list");
                    }
                }

                // FIXME: match on `remote_helper`

                // TODO: use custom transport once commands are implemented
                let mut transport = gitoxide::protocol::transport::connect(
                    url.clone(),
                    gitoxide::protocol::transport::Protocol::V2,
                )
                .await?;

                // Implement once option capability is supported
                let mut progress = gitoxide::progress::Discard;
                let extra_parameters = vec![];

                let outcome = gitoxide::protocol::fetch::handshake(
                    &mut transport,
                    authenticate,
                    extra_parameters,
                    &mut progress,
                )
                .await?;

                let refs = gitoxide::protocol::ls_refs(
                    &mut transport,
                    // outcome.server_protocol_version,
                    &outcome.capabilities,
                    // TODO: gain a better understanding of
                    // https://github.com/Byron/gitoxide/blob/da5f63cbc7506990f46d310f8064678decb86928/git-repository/src/remote/connection/ref_map.rs#L153-L168
                    |_capabilities, _arguments, _features| {
                        Ok(gitoxide::protocol::ls_refs::Action::Continue)
                    },
                    &mut progress,
                )
                .await?;

                trace!("refs: {:#?}", refs);

                // TODO: buffer and flush
                refs.iter().for_each(|r| println!("{}", ref_to_string(r)));
                println!()
            }
            Commands::Push { src_dst } => {
                trace!("batch push {}", src_dst);
                let _ = push.insert(src_dst);
            }
        }
    }
}

fn ref_to_string(r: &gitoxide::protocol::handshake::Ref) -> String {
    use gitoxide::protocol::handshake::Ref;

    match r {
        Ref::Peeled {
            full_ref_name,
            tag: _,
            object: _,
        } => {
            // FIXME: not sure how to handle peeled refs yet
            format!("? {}", full_ref_name)
        }
        Ref::Direct {
            full_ref_name,
            object,
        } => {
            // 91536083cdb16ef3c29638054642b50a34ea8c25 refs/heads/main
            format!("{} {}", object, full_ref_name)
        }
        Ref::Symbolic {
            full_ref_name,
            target,
            object: _,
        } => {
            // @refs/heads/main HEAD
            format!("@{} {}", target, full_ref_name)
        }
        // TODO: determine if this is the correct way to handle unborn symrefs
        Ref::Unborn {
            full_ref_name,
            target,
        } => {
            // @refs/heads/main HEAD
            format!("@{} {}", target, full_ref_name)
        }
    }
}
