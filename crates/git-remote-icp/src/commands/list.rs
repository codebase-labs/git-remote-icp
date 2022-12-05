use clap::ValueEnum;
use git_repository as git;
use log::trace;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
pub enum ListVariant {
    ForPush,
}

pub async fn execute<AuthFn, T>(
    mut transport: T,
    authenticate: AuthFn,
    variant: &Option<ListVariant>,
) -> anyhow::Result<()>
where
    AuthFn: FnMut(git::credentials::helper::Action) -> git::credentials::protocol::Result,
    T: git::protocol::transport::client::Transport,
{
    match variant {
        Some(x) => match x {
            ListVariant::ForPush => trace!("list for-push"),
        },
        None => {
            trace!("list");
        }
    }

    // Implement once option capability is supported
    let mut progress = git::progress::Discard;
    let extra_parameters = vec![];

    let outcome = git::protocol::fetch::handshake(
        &mut transport,
        authenticate,
        extra_parameters,
        &mut progress,
    )
    .await?;

    let refs = git::protocol::ls_refs(
        &mut transport,
        // outcome.server_protocol_version,
        &outcome.capabilities,
        // TODO: gain a better understanding of
        // https://github.com/Byron/gitoxide/blob/da5f63cbc7506990f46d310f8064678decb86928/git-repository/src/remote/connection/ref_map.rs#L153-L168
        |_capabilities, _arguments, _features| Ok(git::protocol::ls_refs::Action::Continue),
        &mut progress,
    )
    .await?;

    trace!("refs: {:#?}", refs);

    // TODO: buffer and flush
    refs.iter().for_each(|r| println!("{}", ref_to_string(r)));
    println!();

    Ok(())
}

fn ref_to_string(r: &git::protocol::handshake::Ref) -> String {
    use git::protocol::handshake::Ref;

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
