use clap::Parser;

#[derive(Debug, Parser)]
#[clap(multicall(true))]
#[clap(about, version)]
pub enum Cli {
    GitRemoteIcp(Args),
    GitRemoteTcp(Args),
}

#[derive(Debug, Parser)]
#[clap(about, version)]
pub struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    pub repository: String,

    /// A URL of the form icp://<address> or icp::<transport>://<address>
    #[clap(value_parser)]
    pub url: String,
}

pub fn args(cli: &Cli) -> &Args {
    match cli {
        Cli::GitRemoteIcp(args) => args,
        Cli::GitRemoteTcp(args) => args,
    }
}
