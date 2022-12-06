use clap::Parser;

#[derive(Debug, Parser)]
#[command(multicall(true))]
#[command(about, version)]
pub enum Cli {
    GitRemoteIcp(Args),
    GitRemoteTcp(Args),
}

#[derive(Debug, Parser)]
#[command(about, version)]
pub struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    pub repository: String,

    /// A URL of the form icp://<address> or icp::<transport>://<address>
    pub url: String,
}

pub fn args(cli: &Cli) -> &Args {
    match cli {
        Cli::GitRemoteIcp(args) => args,
        Cli::GitRemoteTcp(args) => args,
    }
}
