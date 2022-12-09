use git_repository as git;
use log::trace;
use std::collections::BTreeSet;

pub type Batch = BTreeSet<(String, String)>;

pub async fn process<T>(
    transport: T,
    repo: &git::Repository,
    url: &str,
    batch: &mut Batch,
) -> anyhow::Result<()>
where
    T: git::protocol::transport::client::Transport,
{
    if !batch.is_empty() {
        trace!("process fetch: {:#?}", batch);

        let mut remote = repo.remote_at(url)?;

        for (hash, _name) in batch.iter() {
            remote = remote.with_refspecs(Some(hash.as_bytes()), git::remote::Direction::Fetch)?;
        }

        // Implement once option capability is supported
        let progress = git::progress::Discard;

        let outcome = remote
            .to_connection_with_transport(transport, progress)
            .prepare_fetch(git::remote::ref_map::Options {
                prefix_from_spec_as_filter_on_remote: true,
                handshake_parameters: vec![],
            })
            .await?
            .receive(&git::interrupt::IS_INTERRUPTED)
            .await?;

        trace!("outcome: {:#?}", outcome);

        // TODO: delete .keep files by outputting: lock <file>
        // TODO: determine if gitoxide handles this for us yet

        batch.clear();
        println!();
    }

    Ok(())
}
