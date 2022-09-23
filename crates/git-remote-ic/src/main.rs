#![deny(rust_2018_idioms)]

use std::env;

use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum};
use git_protocol::fetch::refs::{self, Ref};
use git_transport::client::http::Transport;
use git_transport::client::Transport as _;
use git_transport::{Protocol, Service};
use log::trace;

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

#[derive(Parser)]
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

#[derive(Clone, ValueEnum)]
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

    loop {
        trace!("loop");

        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("failed to read from stdin with error: {:?}", error))?;

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        let input = input.split(" ").collect::<Vec<_>>();

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
                println!("fetch");
                println!("push");
                println!();
            }
            Commands::Fetch { hash, name } => trace!("fetch {} {}", hash, name),
            Commands::List { variant } => {
                match variant {
                    Some(x) => match x {
                        ListVariant::ForPush => trace!("list for-push"),
                    },
                    None => {
                        trace!("list");

                        // TODO: Provide our own implementation of
                        // git_transport::client::http::Http that does HTTP
                        // message signatures using picky.
                        //
                        // NOTE: It looks like we should be able to reuse
                        // git_transport::client::http::Transport with our
                        // custom Http implementation instead of providing an
                        // implementation for git_transport::client::Transport
                        // as well, but:
                        //
                        // * the feature http-client-curl would still be
                        //   required even though curl wouldn't be involved
                        //
                        // * there is no way to construct a
                        //   git_transport::client::http::Transport because:
                        //
                        //   * Transport::new(_, _) uses Impl::default() which
                        //     where type Impl = curl::Curl, and
                        //
                        //   * the struct fields are private

                        let mut transport = Transport::new(&url, Protocol::V2);
                        let extra_parameters = vec![];
                        let result = transport
                            .handshake(Service::UploadPack, &extra_parameters)
                            .map_err(|e| e.to_string())?;

                        let mut refs = result.refs.ok_or("failed to get refs")?;
                        let parsed_refs =
                            refs::from_v2_refs(&mut refs).map_err(|e| e.to_string())?;
                        trace!("parsed_refs: {:#?}", parsed_refs);

                        // FIXME
                        //
                        // When we make a request to:
                        //
                        //   GET /@paul/hello-world.git/info/refs?service=git-upload-pack
                        //
                        // It returns:
                        //
                        //   0000
                        //   91536083cdb16ef3c29638054642b50a34ea8c25 HEAD\0symref=HEAD:refs/heads/main
                        //   91536083cdb16ef3c29638054642b50a34ea8c25 refs/heads/main
                        //   0000
                        //
                        // We want to produce:
                        //
                        //   @refs/heads/main HEAD
                        //   91536083cdb16ef3c29638054642b50a34ea8c25 refs/heads/main
                        //
                        // But parsed_refs only contains Direct refs when we
                        // expect a Symbolic ref

                        parsed_refs
                            .iter()
                            .for_each(|r| println!("{}", ref_to_string(r)));
                        println!()
                    }
                }
            }
            Commands::Push { src_dst } => trace!("push {}", src_dst),
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
