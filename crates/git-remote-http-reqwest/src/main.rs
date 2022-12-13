mod connect;

use connect::connect;
use git_remote_helper;

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    git_remote_helper::main(connect)
}
