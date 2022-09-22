#![deny(rust_2018_idioms)]

use clap::Parser;

#[derive(Parser)]
#[clap(about, version)]
struct Args {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    repository: String,

    /// A URL of the form ic://<address> or ic::<transport>://<address>
    #[clap(value_parser)]
    url: Option<String>,
}

enum Commands {
    Capabiliites,
    List,
}

fn main() {
    let args = Args::parse();
    println!("repository: {:?}", args.repository);
    println!("url: {:?}", args.url);
}
