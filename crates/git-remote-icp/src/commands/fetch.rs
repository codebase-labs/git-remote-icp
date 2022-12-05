use git_repository as git;
use log::trace;
use std::collections::BTreeSet;

pub type Batch = BTreeSet<(String, String)>;

pub async fn process(repo: &git::Repository, url: &str, batch: &mut Batch) -> anyhow::Result<()> {
    if !batch.is_empty() {
        trace!("process fetch: {:#?}", batch);

        let mut remote = repo.remote_at(url)?;

        for (hash, _name) in batch.iter() {
            remote = remote.with_refspec(hash.as_bytes(), git::remote::Direction::Fetch)?;
        }

        // Implement once option capability is supported
        let progress = git::progress::Discard;

        // FIXME: match on `remote_helper`

        // TODO: use custom transport once commands are implemented
        let transport =
            git::protocol::transport::connect(url, git::protocol::transport::Protocol::V2)
                .await?;

        let outcome = remote
            .to_connection_with_transport(transport, progress)
            // For pushing we should get the packetline writer here
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
