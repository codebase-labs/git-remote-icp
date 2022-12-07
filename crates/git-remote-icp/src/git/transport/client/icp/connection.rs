use git::protocol::transport;
use git_repository as git;
use ic_agent::{Agent, Identity};
use std::sync::Arc;

pub struct Connection {
    pub agent: Agent,
    pub desired_version: transport::Protocol,
}

impl Connection {
    pub fn new(
        identity: Arc<dyn Identity>,
        desired_version: transport::Protocol,
    ) -> Result<Self, transport::connect::Error> {
        let agent = Agent::builder()
            .with_arc_identity(identity.clone())
            .build()
            .map_err(|err| transport::connect::Error::Connection(Box::new(err)))?;
        let connection = Self {
            agent,
            desired_version,
        };
        Ok(connection)
    }
}
