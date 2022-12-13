#![feature(async_closure)]
#![feature(impl_trait_in_fn_trait_return)]

mod connect;

use git_remote_helper;
use connect::connect;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    env_logger::init();
    git_remote_helper::main(connect).await
}
