use clap::Parser;
use strum::EnumVariantNames;

pub mod fetch;
pub mod list;
pub mod push;

use list::ListVariant;

#[derive(Debug, EnumVariantNames, Eq, Ord, PartialEq, PartialOrd, Parser)]
#[strum(serialize_all = "kebab_case")]
pub enum Commands {
    Capabilities,
    Fetch {
        #[clap(value_parser)]
        hash: String, // TODO: gitoxide::hash::ObjectId?

        #[clap(value_parser)]
        name: String,
    },
    List {
        #[clap(arg_enum, value_parser)]
        variant: Option<ListVariant>,
    },
    Push {
        #[clap(value_parser)]
        src_dst: String,
    },
}
