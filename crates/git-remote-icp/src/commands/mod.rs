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
        hash: String, // TODO: gitoxide::hash::ObjectId?

        name: String,
    },
    List {
        variant: Option<ListVariant>,
    },
    Push {
        src_dst: String,
    },
}
