#![deny(rust_2018_idioms)]

use clap::{Command, FromArgMatches as _, Parser, Subcommand as _, ValueEnum };

#[derive(Parser)]
#[clap(about, version)]
struct Cli {
    /// A remote repository; either the name of a configured remote or a URL
    #[clap(value_parser)]
    repository: String,

    /// A URL of the form ic://<address> or ic::<transport>://<address>
    #[clap(value_parser)]
    url: String,
}

#[derive(Parser)]
enum Commands {
    Capabilities,
    Fetch,
    List {
        #[clap(arg_enum, value_parser)]
        variant: Option<ListVariant>,
    },
    Push,
}

#[derive(Clone, ValueEnum)]
enum ListVariant {
  ForPush,
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    eprintln!("cli.repository: {:?}", cli.repository);
    eprintln!("cli.url: {:?}", cli.url);

    let url: String = match cli.url.strip_prefix("ic://") {
        // The supplied URL was of the form `ic://<address>` so we change it to
        // `https://<address>`
        Some(address) => format!("https://{}", address),
        // The supplied url was of the form `ic::<transport>://<address>` but
        // Git invoked the remote helper with `<transport>://<address>`
        None => cli.url.to_string(),
    };

    eprintln!("url: {}", url);

    loop {
        eprintln!("loop");

        let mut input = String::new();

        std::io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("failed to read from stdin with error: {:?}", error))?;

        let input = input.trim();
        let input = input.split(" ").collect::<Vec<_>>();

        eprintln!("input: {:#?}", input);

        let input_command = Command::new("input")
            .multicall(true)
            .subcommand_required(true);

        let input_command = Commands::augment_subcommands(input_command);

        let matches = input_command
            .try_get_matches_from(input)
            .map_err(|e| e.to_string())?;

        let command = Commands::from_arg_matches(&matches).map_err(|e| e.to_string())?;

        match command {
            Commands::Capabilities => {
                println!("fetch");
                println!("push");
                println!();
            },
            Commands::Fetch => eprintln!("got: fetch"),
            Commands::List { variant } => {
                match variant {
                    Some(x) => match x {
                        ListVariant::ForPush => eprintln!("got: list for-push"),
                    }
                    None => eprintln!("got: list"),
                }
            },
            Commands::Push => eprintln!("got: push"),
        }
    }
}
