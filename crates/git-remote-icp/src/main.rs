#![deny(rust_2018_idioms)]

use anyhow::{anyhow, Context};
use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum};
use git_features::progress;
use git_protocol::fetch;
use git_protocol::fetch::refs::Ref;
use log::trace;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use strum::{EnumVariantNames, VariantNames as _};

#[derive(Parser)]
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
        hash: String, // TODO: git_hash::ObjectId?

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    git_repository::interrupt::init_handler(move || {})?;

    let git_dir = env::var(GIT_DIR).context("failed to get GIT_DIR")?;

    trace!("GIT_DIR: {}", git_dir);

    // TODO: determine if we can use gitoxide here instead
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

    let args = Args::parse();
    trace!("args.repository: {:?}", args.repository);
    trace!("args.url: {:?}", args.url);

    let url: String = match args.url.strip_prefix("icp://") {
        // The supplied URL was of the form `icp://<address>` so we change it to
        // `git://<address>` (for now)
        Some(address) => format!("git://{}", address),
        // The supplied url was of the form `icp::<transport>://<address>` but
        // Git invoked the remote helper with `<transport>://<address>`
        None => args.url.to_string(),
    };

    trace!("url: {}", url);

    let repo_dir = Path::new(&git_dir)
        .parent()
        .ok_or(anyhow!("failed to get repository directory"))?;

    let repo = git_repository::open(repo_dir)?;

    let authenticate =
        |action| panic!("unexpected call to authenticate with action: {:#?}", action);

    let mut fetch: BTreeSet<(String, String)> = BTreeSet::new();
    let mut push: BTreeSet<String> = BTreeSet::new();

    loop {
        trace!("loop");

        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .context("failed to read from stdin")?;

        let input = input.trim();

        if input.is_empty() {
            trace!("terminated with a blank line");

            if !fetch.is_empty() {
                trace!("process fetch: {:#?}", fetch);

                let mut remote = repo.remote_at(url.clone())?;

                for (hash, _name) in &fetch {
                    remote = remote
                        .with_refspec(hash.as_bytes(), git_repository::remote::Direction::Fetch)?;
                }

                // Implement once option capability is supported
                let progress = progress::Discard;

                // TODO: use custom transport once commands are implemented
                let transport = git_transport::connect(url, git_transport::Protocol::V2).await?;

                let outcome = remote
                    .to_connection_with_transport(transport, progress)
                    // For pushing we should get the packetline writer here
                    .prepare_fetch(git_repository::remote::ref_map::Options {
                        prefix_from_spec_as_filter_on_remote: true,
                        handshake_parameters: vec![],
                    })
                    .await?
                    .receive(&git_repository::interrupt::IS_INTERRUPTED)
                    .await?;

                trace!("outcome: {:#?}", outcome);

                // TODO: delete .keep files by outputting: lock <file>
                // TODO: determine if gitoxide handles this for us yet

                fetch.clear();
                println!();
            }

            if !push.is_empty() {
                trace!("process push: {:#?}", push);

                // push.clear();
                // println!();
            }

            // continue; // Useful to inspect .git directory before it disappears
            break Ok(());
        }

        let input = input.split(' ').collect::<Vec<_>>();

        trace!("input: {:#?}", input);

        let input_command = Command::new("input")
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

                // TODO: use custom transport once commands are implemented
                let mut transport =
                    git_transport::connect(url.clone(), git_transport::Protocol::V2).await?;

                // Implement once option capability is supported
                let mut progress = progress::Discard;
                let extra_parameters = vec![];

                let outcome = fetch::handshake(
                    &mut transport,
                    authenticate,
                    extra_parameters,
                    &mut progress,
                )
                .await?;

                let refs = fetch::refs(
                    &mut transport,
                    outcome.server_protocol_version,
                    &outcome.capabilities,
                    // TODO: gain a better understanding of
                    // https://github.com/Byron/gitoxide/blob/da5f63cbc7506990f46d310f8064678decb86928/git-repository/src/remote/connection/ref_map.rs#L153-L168
                    |_capabilities, _arguments, _features| {
                        Ok(fetch::delegate::LsRefsAction::Continue)
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

fn ref_to_string(r: &Ref) -> String {
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