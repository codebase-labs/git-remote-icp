// TODO: determine what to name this module
mod reqwest;

pub use self::reqwest::Remote;

use git_repository as git;
pub use git::protocol::transport::client::http::*;
