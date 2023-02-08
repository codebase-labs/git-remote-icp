use crate::git::service::receive_pack;
use anyhow::anyhow;
use git::bstr::ByteSlice as _;
use git::odb::pack::data::output::count::objects::ObjectExpansion;
use git_repository as git;
use log::trace;
use maybe_async::maybe_async;
use std::collections::BTreeSet;

#[cfg(feature = "blocking-network-client")]
use std::io::Write as _;

#[cfg(feature = "async-network-client")]
use git::protocol::futures_lite::io::AsyncWriteExt as _;

pub type Batch = BTreeSet<String>;

#[maybe_async]
pub async fn process<AuthFn, T>(
    mut transport: T,
    repo: &git::Repository,
    authenticate: AuthFn,
    batch: &mut Batch,
) -> anyhow::Result<()>
where
    AuthFn: FnMut(git::credentials::helper::Action) -> git::credentials::protocol::Result,
    T: git::protocol::transport::client::Transport,
{
    if !batch.is_empty() {
        trace!("process push: {:#?}", batch);

        use git::refspec::parse::Operation;
        use git::refspec::{instruction, Instruction};

        // Implement once option capability is supported
        let mut progress = git::progress::Discard;
        let extra_parameters = vec![];

        let mut outcome = git::protocol::handshake(
            &mut transport,
            git::protocol::transport::Service::ReceivePack,
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

        let mut request_writer = transport.request(
            git::protocol::transport::client::WriteMode::Binary,
            // This is currently redundant because we use `.into_parts()`
            git::protocol::transport::client::MessageKind::Flush,
        )?;

        let instructions = batch
            .iter()
            .map(|unparse_ref_spec| {
                let ref_spec_ref =
                    git::refspec::parse(unparse_ref_spec.as_bytes().as_bstr(), Operation::Push)?;
                Ok(ref_spec_ref.instruction())
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;

        trace!("instructions: {:#?}", instructions);

        let push_instructions = instructions
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
        let input_object_expansion = ObjectExpansion::TreeAdditionsComparedToAncestor;

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
                .unwrap_or_else(|| git::hash::Kind::Sha1.null());

            trace!("dst_id: {:#?}", dst_id);

            let dst_object = repo.find_object(dst_id)?;
            let dst_commit = dst_object.try_into_commit()?;
            let dst_commit_time = dst_commit
                .committer()
                .map(|committer| committer.time.seconds_since_unix_epoch)?;

            let ancestors = src_id
                .ancestors()
                .sorting(
                    git::traverse::commit::Sorting::ByCommitTimeNewestFirstCutoffOlderThan {
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
                git::odb::pack::data::output::count::objects_unthreaded(
                    db.clone(),
                    commits.into_iter(),
                    // Implement once option capability is supported
                    git::progress::Discard,
                    &git::interrupt::IS_INTERRUPTED,
                    input_object_expansion,
                )?;

            counts.shrink_to_fit();

            trace!("counts: {:#?}", counts);

            // TODO: in order iter
            let mut entries_iter = git::odb::pack::data::output::entry::iter_from_counts(
                counts,
                db,
                git::progress::Discard,
                git::odb::pack::data::output::entry::iter_from_counts::Options {
                    allow_thin_pack: false,
                    ..Default::default()
                },
            );

            entries.push(
                git::parallel::InOrderIter::from(entries_iter.by_ref())
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
            );

            // NOTE: We send `report-status-v2` so that we receive a response
            // that includes a status report.
            //
            // We parse this and write a status report to stdout in the format
            // that remote helpers are expected to produce.
            let chunk = format!(
                "{} {} {}\0 report-status-v2 \n",
                dst_id.to_hex(),
                src_id.to_hex(),
                dst
            );

            request_writer.write_all(chunk.as_bytes().as_bstr()).await?;
        }

        request_writer
            .write_message(git::protocol::transport::client::MessageKind::Flush)
            .await?;

        let entries = entries.into_iter().flatten().collect::<Vec<_>>();
        trace!("entries: {:#?}", entries);

        let num_entries: u32 = entries.len().try_into()?;
        trace!("num entries: {:#?}", num_entries);

        let (mut writer, reader) = request_writer.into_parts();

        #[cfg(feature = "async-network-client")]
        let mut writer = git::protocol::futures_lite::io::BlockOn::new(&mut writer);

        let pack_writer = git::odb::pack::data::output::bytes::FromEntriesIter::new(
            std::iter::once(Ok::<
                _,
                git::odb::pack::data::output::entry::iter_from_counts::Error<
                    git::odb::store::find::Error,
                >,
            >(entries)),
            &mut writer,
            num_entries,
            git::odb::pack::data::Version::V2,
            git::hash::Kind::Sha1,
        );

        // The pack writer is lazy, so we need to consume it
        for write_result in pack_writer {
            let bytes_written = write_result?;
            trace!("bytes written: {:#?}", bytes_written);
        }

        // Signal that we are done writing
        drop(writer);

        trace!("finished writing pack");


        let (_unpack_result, command_statuses) =
            receive_pack::response::read_and_parse(reader).await?;

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

        batch.clear();

        // Terminate the status report output
        println!();
    }

    Ok(())
}
