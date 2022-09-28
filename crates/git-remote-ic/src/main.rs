#![deny(rust_2018_idioms)]

use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum};
use git_protocol::fetch::refs::{self, Ref};
use git_transport::client::{http, Transport};
use git_transport::Service;
use gitoxide_core as core;
use log::trace;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use strum::{EnumString, EnumVariantNames, VariantNames as _};

// use http::header::{self, HeaderName};
// use http::method::Method;
// use http::request;

// use picky::hash::HashAlgorithm;
// use picky::http::http_signature::{HttpSignature, HttpSignatureBuilder};
// use picky::key::PrivateKey;
// use picky::pem::parse_pem;
// use picky::signature::SignatureAlgorithm;

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

#[derive(Debug, EnumString, EnumVariantNames, Parser)]
#[strum(serialize_all = "kebab_case")]
enum Commands {
    #[strum(disabled)]
    Capabilities,
    Fetch {
        #[clap(value_parser)]
        hash: String, // TODO: git_hash::ObjectId?

        #[clap(value_parser)]
        name: String,
    },
    #[strum(disabled)]
    List {
        #[clap(arg_enum, value_parser)]
        variant: Option<ListVariant>,
    },
    Push {
        #[clap(value_parser)]
        src_dst: String,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum ListVariant {
    ForPush,
}

const GIT_DIR: &str = "GIT_DIR";

#[tokio::main]
async fn main() -> Result<(), String> {
    env_logger::init();

    let git_dir =
        env::var(GIT_DIR).map_err(|e| format!("failed to get GIT_DIR with error: {}", e))?;

    trace!("GIT_DIR: {}", git_dir);

    let args = Args::parse();
    trace!("args.repository: {:?}", args.repository);
    trace!("args.url: {:?}", args.url);

    let url: String = match args.url.strip_prefix("ic://") {
        // The supplied URL was of the form `ic://<address>` so we change it to
        // `https://<address>`
        Some(address) => format!("https://{}", address),
        // The supplied url was of the form `ic::<transport>://<address>` but
        // Git invoked the remote helper with `<transport>://<address>`
        None => args.url.to_string(),
    };

    trace!("url: {}", url);

    let mut batch: Vec<Commands> = vec![];

    loop {
        trace!("loop");

        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("failed to read from stdin with error: {:?}", error))?;

        let input = input.trim();

        if input.is_empty() {
            trace!("terminated with a blank line");
            trace!("process batch: {:#?}", batch);
            // TODO: actually process the batch
            batch.clear();
            continue;
        }

        let input = input.split(' ').collect::<Vec<_>>();

        trace!("input: {:#?}", input);

        let input_command = Command::new("input")
            .multicall(true)
            .subcommand_required(true);

        let input_command = Commands::augment_subcommands(input_command);

        let matches = input_command
            .try_get_matches_from(input)
            .map_err(|e| e.to_string())?;

        let command = Commands::from_arg_matches(&matches).map_err(|e| e.to_string())?;

        match command {
            Commands::Capabilities => {
                // TODO: buffer and flush
                Commands::VARIANTS
                    .iter()
                    .for_each(|command| println!("{}", command));
                println!();
            }
            Commands::Fetch { ref hash, ref name } => {
                trace!("batch fetch {} {}", hash, name);
                batch.push(command)
            }
            Commands::List { variant } => {
                match variant {
                    Some(x) => match x {
                        ListVariant::ForPush => trace!("list for-push"),
                    },
                    None => {
                        trace!("list");

                        // Using the following approach for now because we can't
                        // seem to easily construct a delegate to pass to
                        // git_protocol::fetch
                        //
                        // * Delegate impls in git-protocol are only for tests
                        // * Delegate impl in gitoxide-core is private

                        let protocol = core::net::Protocol::V1; // FIXME: use v2
                        let refs_directory = Some(PathBuf::from(GIT_DIR));
                        let wanted_refs = Vec::<BString>::new(); // Fetch all advertised references
                        let pack_and_index_directory = Some(PathBuf::from(GIT_DIR));
                        let progress = git_features::progress::Discard;

                        let thread_limit = None;
                        let format = core::OutputFormat::Human;
                        let should_interrupt = Arc::new(AtomicBool::new(false));
                        let mut out = Vec::<u8>::new();
                        // let object_hash = git_repository::hash::Kind::SHA1;
                        let object_hash = git_hash::Kind::Sha1;

                        let context = core::pack::receive::Context {
                            thread_limit,
                            format,
                            should_interrupt,
                            out,
                            object_hash,
                        };

                        let _ = core::pack::receive(
                            Some(protocol),
                            &url,
                            pack_and_index_directory,
                            refs_directory,
                            wanted_refs.into_iter().map(|r| r.into()).collect(),
                            progress,
                            context,
                        );

                        let mut refs = result.refs.ok_or("failed to get refs")?;
                        let capabilities = result.capabilities.iter();

                        // FIXME: use v2
                        let parsed_refs =
                            // refs::from_v2_refs(&mut refs).map_err(|e| e.to_string())?;
                            refs::from_v1_refs_received_as_part_of_handshake_and_capabilities(&mut refs, capabilities).map_err(|e| e.to_string())?;

                        trace!("parsed_refs: {:#?}", parsed_refs);

                        // TODO: buffer and flush
                        parsed_refs
                            .iter()
                            .for_each(|r| println!("{}", ref_to_string(r)));
                        println!()
                    }
                }
            }
            Commands::Push { ref src_dst } => {
                trace!("batch push {}", src_dst);
                batch.push(command)
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
            // TODO: confirm these are the right way around
            format!("@{} {}", full_ref_name, target)
        }
    }
}
