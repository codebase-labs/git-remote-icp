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
    let mut push: BTreeSet<String> = BTreeSet::new();

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

            if !push.is_empty() {
                trace!("process push: {:#?}", push);

                use gitoxide::refspec::parse::Operation;
                use gitoxide::refspec::{instruction, Instruction};

                // FIXME: match on `remote_helper`

                // TODO: use custom transport once commands are implemented
                // NOTE: push still uses the v1 protocol so we use that here.
                let mut transport = gitoxide::protocol::transport::connect(
                    url.clone(),
                    gitoxide::protocol::transport::Protocol::V1,
                )
                .await?;

                // Implement once option capability is supported
                let mut progress = gitoxide::progress::Discard;
                let extra_parameters = vec![];

                let mut outcome = gitoxide::protocol::handshake(
                    &mut transport,
                    gitoxide::protocol::transport::Service::ReceivePack,
                    authenticate,
                    extra_parameters,
                    &mut progress,
                )
                .await?;

                let remote_refs = outcome
                    .refs
                    .take()
                    .ok_or_else(|| anyhow!("failed to take remote refs"))?;

                trace!("remote_refs: {:#?}", remote_refs);

                let mut writer = transport.request(
                    gitoxide::protocol::transport::client::WriteMode::Binary,
                    // This is currently redundant because we use `.into_parts()`
                    gitoxide::protocol::transport::client::MessageKind::Flush,
                )?;

                let instructions = push
                    .iter()
                    .map(|unparse_ref_spec| {
                        let ref_spec_ref = gitoxide::refspec::parse(
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
                let input_object_expansion = gitoxide::odb::pack::data::output::count::objects::ObjectExpansion::TreeAdditionsComparedToAncestor;

                // TODO: combine with previous iterations if possible
                // TODO: for loop instead of map
                let mut entries = vec![];

                for (src, dst, _allow_non_fast_forward) in push_instructions {
                    // local
                    let mut src_reference = repo.find_reference(*src)?;
                    let src_id = src_reference.peel_to_id_in_place()?;

                    // remote
                    let dst_id = remote_refs
                        .iter()
                        .find_map(|r| {
                            let (name, target, peeled) = r.unpack();
                            (name == *dst).then(|| peeled.or(target)).flatten()
                        })
                        .map(|x| x.to_owned())
                        .unwrap_or_else(|| gitoxide::hash::Kind::Sha1.null());

                    trace!("dst_id: {:#?}", dst_id);

                    let dst_object = repo.find_object(dst_id)?;
                    let dst_commit = dst_object.try_into_commit()?;
                    let dst_commit_time = dst_commit
                        .committer()
                        .map(|committer| committer.time.seconds_since_unix_epoch)?;

                    let ancestors = src_id
                        .ancestors()
                        .sorting(
                            gitoxide::traverse::commit::Sorting::ByCommitTimeNewestFirstCutoffOlderThan {
                                time_in_seconds_since_epoch: dst_commit_time,
                            },
                        )
                        // TODO: repo object cache?
                        .all()
                        // NOTE: this is suboptimal but makes debugging easier
                        .map(|ancestor_commits| ancestor_commits.collect::<Vec<_>>());

                    trace!("ancestors: {:#?}", ancestors);

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

                    // NOTE: we don't want to short circuit on this Result
                    // until after we've determined if we can fast-forward.
                    let commits = ancestors?;

                    let (mut counts, _count_stats) =
                        gitoxide::odb::pack::data::output::count::objects_unthreaded(
                            db.clone(),
                            commits.into_iter(),
                            // Implement once option capability is supported
                            gitoxide::progress::Discard,
                            &gitoxide::interrupt::IS_INTERRUPTED,
                            input_object_expansion,
                        )?;

                    counts.shrink_to_fit();

                    trace!("counts: {:#?}", counts);

                    // TODO: in order iter
                    let mut entries_iter = gitoxide::odb::pack::data::output::entry::iter_from_counts(
                        counts,
                        db,
                        gitoxide::progress::Discard,
                        gitoxide::odb::pack::data::output::entry::iter_from_counts::Options {
                            allow_thin_pack: false,
                            ..Default::default()
                        },
                    );

                    entries.push(
                        gitoxide::parallel::InOrderIter::from(entries_iter.by_ref())
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>(),
                    );

                    // NOTE
                    //
                    // * We send `report-status-v2` so that we receive a
                    //   response that includes a status report. We parse this
                    //   and write a status report to stdout in the format that
                    //   remote helpers are expected to produce.
                    //
                    // * See comments on reading the `receive-pack` response as
                    //   to why we send the sideband capability.
                    let chunk = format!(
                        "{} {} {}\0 report-status-v2 side-band-64k",
                        dst_id.to_hex(),
                        src_id.to_hex(),
                        dst
                    );

                    use gitoxide::protocol::futures_lite::io::AsyncWriteExt as _;
                    writer.write_all(chunk.as_bytes().as_bstr()).await?;
                }

                writer
                    .write_message(gitoxide::protocol::transport::client::MessageKind::Flush)
                    .await?;

                let entries = entries.into_iter().flatten().collect::<Vec<_>>();
                trace!("entries: {:#?}", entries);

                let num_entries: u32 = entries.len().try_into()?;
                trace!("num entries: {:#?}", num_entries);

                let (mut async_writer, mut async_reader) = writer.into_parts();

                let mut sync_writer =
                    gitoxide::protocol::futures_lite::io::BlockOn::new(&mut async_writer);

                let pack_writer = gitoxide::odb::pack::data::output::bytes::FromEntriesIter::new(
                    std::iter::once(Ok::<
                        _,
                        gitoxide::odb::pack::data::output::entry::iter_from_counts::Error<
                            gitoxide::odb::store::find::Error,
                        >,
                    >(entries)),
                    &mut sync_writer,
                    num_entries,
                    gitoxide::odb::pack::data::Version::V2,
                    gitoxide::hash::Kind::Sha1,
                );

                // The pack writer is lazy, so we need to consume it
                for write_result in pack_writer {
                    let bytes_written = write_result?;
                    trace!("bytes written: {:#?}", bytes_written);
                }

                trace!("finished writing pack");

                // If we don't send any sideband capabilities, we get
                // `Some(Err(Kind(UnexpectedEof)))` in the `AsyncBufRead`
                // implementation for `WithSidebands` here when trying to read
                // the `receive-pack` response:
                // https://github.com/paulyoung/gitoxide/blob/93f2dd8f7db87afc04a523458faaa46f9b33f21a/git-packetline/src/read/sidebands/async_io.rs#L213
                //
                // So, we send `side-band-64k` to address that. Even though we
                // currently don't support reporting any progress, we set a
                // progress handler to keep the sideband information separate
                // from the response we care about.
                use std::ops::Deref as _;
                use std::sync::{Arc, Mutex};
                let messages = Arc::new(Mutex::new(Vec::<String>::new()));
                async_reader.set_progress_handler(Some(Box::new({
                    move |is_err, data| {
                        assert!(!is_err);
                        messages
                            .deref()
                            .lock()
                            .expect("no panic in other threads")
                            .push(std::str::from_utf8(data).expect("valid utf8").to_owned())
                    }
                })));

                let mut streaming_peekable_iter =
                    gitoxide::protocol::transport::packetline::StreamingPeekableIter::new(
                        async_reader,
                        &[gitoxide::protocol::transport::packetline::PacketLineRef::Flush],
                    );

                streaming_peekable_iter.fail_on_err_lines(true);
                let mut reader = streaming_peekable_iter.as_read();

                let (_unpack_result, command_statuses) =
                    receive_pack::response::read_and_parse(&mut reader).await?;

                command_statuses.iter().for_each(|command_status| {
                    trace!("{:#?}", command_status);
                    match command_status {
                        receive_pack::response::CommandStatusV2::Ok(ref_name, _option_lines) => {
                            let output = format!("ok {}", ref_name);
                            trace!("output: {}", output);
                            println!("{}", output);
                        }
                        receive_pack::response::CommandStatusV2::Fail(ref_name, error_msg) => {
                            let output = format!("error {} {}\0", ref_name, error_msg);
                            trace!("output: {}", output);
                            println!("{}", output);
                        }
                    }
                });

                push.clear();

                // Terminate the status report output
                println!();
            }

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
