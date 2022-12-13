use clap::Parser;

#[derive(Debug, Parser)]
#[command(about, version)]
pub struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    pub repository: String,

    /// A URL of the form icp://<address> or icp::<transport>://<address>
    pub url: String,
}
