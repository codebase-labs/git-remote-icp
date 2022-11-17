#![deny(rust_2018_idioms)]

use anyhow::{anyhow, Context};
use bstr::ByteSlice as _;
use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum};
use git_features::parallel::InOrderIter;
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
        .ok_or_else(|| anyhow!("failed to get repository directory"))?;

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
                let transport =
                    git_transport::connect(url.clone(), git_transport::Protocol::V2).await?;

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

                use git_refspec::parse::Operation;
                use git_refspec::{instruction, Instruction};

                let mut remote = repo.remote_at(url.clone())?;

                // TODO: use custom transport once commands are implemented
                // NOTE: push still uses the v1 protocol so we use that here.
                let mut transport =
                    git_transport::connect(url.clone(), git_transport::Protocol::V1).await?;

                // Implement once option capability is supported
                let mut progress = progress::Discard;
                let extra_parameters = vec![];

                let mut outcome = fetch::handshake(
                    &mut transport,
                    authenticate,
                    extra_parameters,
                    &mut progress,
                )
                .await?;

                let remote_refs = outcome
                    .refs
                    .take()
                    .expect("there should always be refs with v1 protocol");

                trace!("remote_refs: {:#?}", remote_refs);

                // HACK: fetch::handshake uses
                // git_transport::Service::UploadPack instead of ReceivePack so
                // we need this hack for now.
                let mut transport =
                    git_transport::connect(url.clone(), git_transport::Protocol::V1).await?;

                let writer = transport.request(
                    git_transport::client::WriteMode::Binary,
                    git_transport::client::MessageKind::Flush,
                )?;

                let (mut async_write, mut async_read) = writer.into_parts();

                let instructions = push
                    .iter()
                    .map(|unparse_ref_spec| {
                        let ref_spec_ref = git_refspec::parse(
                            unparse_ref_spec.as_bytes().as_bstr(),
                            Operation::Push,
                        )?;
                        Ok(ref_spec_ref.instruction())
                    })
                    .collect::<Result<Vec<_>, anyhow::Error>>()?;

                trace!("instructions: {:#?}", instructions);

                let push_instructions =
                    instructions
                        .iter()
                        .filter_map(|instruction| match instruction {
                            Instruction::Push(instruction::Push::Matching {
                                src,
                                dst,
                                allow_non_fast_forward,
                            }) => Some((src, dst, allow_non_fast_forward)),
                            _ => None,
                        });

                trace!("push instructions: {:#?}", push_instructions);

                // TODO: use Traverse for initial push
                let input_object_expansion = git_pack::data::output::count::objects::ObjectExpansion::TreeAdditionsComparedToAncestor;

                // TODO: consider making this part of the for loop since we
                // can't clone it anyway.
                let ancestors = push_instructions
                    .map(|(src, dst, allow_non_fast_forward)| {
                        // local
                        let mut src_reference = repo.find_reference(*src)?;
                        let src_id = src_reference.peel_to_id_in_place()?;

                        // remote
                        let dst_id = remote_refs.iter()
                            .find_map(|r| {
                                let (name, target, peeled) = r.unpack();
                                (name == *dst).then(|| peeled.or(target)).flatten()
                            })
                            .map(|x| x.to_owned())
                            .unwrap_or_else(|| git_hash::Kind::Sha1.null());

                        trace!("dst_id: {:#?}", dst_id);

                        let dst_object = repo.find_object(dst_id)?;
                        let dst_commit = dst_object.try_into_commit()?;
                        let dst_commit_time = dst_commit
                            .committer()
                            .map(|committer| committer.time.seconds_since_unix_epoch)?;

                        let ancestors = src_id
                            .ancestors()
                            .sorting(
                                git_traverse::commit::Sorting::ByCommitTimeNewestFirstCutoffOlderThan {
                                    time_in_seconds_since_epoch: dst_commit_time
                                },
                            )
                            // TODO: repo object cache?
                            .all()
                            // NOTE: this is suboptimal but makes debugging easier
                            .map(|ancestor_commits| ancestor_commits.collect::<Vec<_>>());

                        // FIXME: it appears that we want to return the
                        // references instead of the IDs so that we can later
                        // use the name in the case of a symbolic reference.
                        Ok((src, dst, allow_non_fast_forward, src_id, dst_id, ancestors))
                    })
                    .collect::<Result<Vec<_>, anyhow::Error>>()?;

                trace!("ancestors: {:#?}", ancestors);

                for (_src, dst, _allow_non_fast_forward, src_id, dst_id, ancestor_commits) in ancestors {
                    // FIXME: We need to handle fast-forwards and force pushes.
                    // Ideally we'd fail fast but we can't because figuring out
                    // if a fast-forward is possible consumes the
                    // `ancestor_commits` iterator which can't be cloned.
                    //
                    // TODO: Investigate if we can do this after we're otherwise
                    // done with `ancestor_commits`.
                    /*
                    let is_fast_forward = match ancestor_commits {
                        Ok(mut commits) => commits.any(|commit_id| {
                            commit_id.map_or(false, |commit_id| commit_id == dst_id)
                        }),
                        Err(_) => false,
                    };

                    trace!("is_fast_forward: {:#?}", is_fast_forward);
                    trace!("allow_non_fast_forward: {:#?}", allow_non_fast_forward);

                    if !is_fast_forward && !allow_non_fast_forward {
                        return Err(anyhow!("attempted non fast-forward push without force"));
                    }
                    */

                    // TODO: set_pack_cache?
                    // TODO: ignore_replacements?
                    let mut db = repo.objects.clone();
                    db.prevent_pack_unload();

                    let commits = ancestor_commits?;

                    let (mut counts, _count_stats) =
                        git_pack::data::output::count::objects_unthreaded(
                            db.clone(),
                            commits.into_iter(),
                            // Implement once option capability is supported
                            progress::Discard,
                            &git_repository::interrupt::IS_INTERRUPTED,
                            input_object_expansion,
                        )?;

                    counts.shrink_to_fit();

                    trace!("counts: {:#?}", counts);

                    // TODO: in order iter
                    let mut entries_iter = git_pack::data::output::entry::iter_from_counts(
                        counts,
                        db,
                        progress::Discard,
                        git_pack::data::output::entry::iter_from_counts::Options {
                            allow_thin_pack: false,
                            ..Default::default()
                        },
                    );

                    let entries = InOrderIter::from(entries_iter.by_ref())
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();

                    // TODO: writer.write_all(b"").await?;
                    // writer.write_message(git_transport::client::MessageKind::Flush).await?;

                    let chunk = format!("{} {} {}", dst_id.to_hex(), src_id.to_hex(), dst);
                    // let chunk = format!("{} {} {}\0{}", dst, src, dst_name, capabilities);
                    git_packetline::encode::text_to_write(
                        chunk.as_bytes().as_bstr(),
                        &mut async_write,
                    )
                    .await?;

                    let mut write = git_protocol::futures_lite::io::BlockOn::new(&mut *async_write);
                    let num_entries: u32 = entries.len().try_into()?;

                    // TODO: ensure that we only send 1 pack per request, this might impact batching
                    let pack_writer = git_pack::data::output::bytes::FromEntriesIter::new(
                        std::iter::once(Ok::<
                            _,
                            git_pack::data::output::entry::iter_from_counts::Error<
                                git_odb::store::find::Error,
                            >,
                        >(entries)),
                        &mut write,
                        num_entries,
                        git_pack::data::Version::V2,
                        git_hash::Kind::Sha1,
                    );

                    for write_result in pack_writer {
                       let _bytes_written = write_result?;
                    }
                }

                git_packetline::encode::flush_to_write(&mut async_write).await?;

                use git_protocol::futures_lite::io::AsyncBufReadExt as _;
                let mut lines = (&mut *async_read).lines();

                let mut info = vec![];
                use git_protocol::futures_lite::StreamExt as _;

                // FIXME: because we don't set up the progress handler, we
                // will also get sideband (if we tell the server we want
                // sideband)

                while let Some(line) = lines.next().await {
                    info.push(line?)
                }

                trace!("info: {:#?}", info);

                // TEMP: Don't successfully exit until this command is implemented

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
                        // TODO: do handshake, keep connection, keep refs for
                        // the push command
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
