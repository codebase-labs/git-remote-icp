use git::protocol::transport;
use ic_agent::export::Principal;
use ic_agent::identity::Identity;
use git_repository as git;
use std::sync::Arc;
use std::future::Future;

pub fn make<'a, Url, E>(
    identity: Arc<dyn Identity>,
    fetch_root_key: bool,
    replica_url: &str,
    canister_id: Principal,
) -> impl Fn(Url, transport::Protocol) -> (impl Future<Output = Result<Box<(dyn transport::client::Transport + Send + 'a)>, transport::connect::Error>> + 'a)
where
    Url: TryInto<git::url::Url, Error = E>,
    git::url::parse::Error: From<E>,
{
    let connect = async move |url, desired_version| {
        todo!()
    };
    connect
}
