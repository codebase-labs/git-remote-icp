use git::protocol::transport::Protocol;
use git_repository as git;
use ic_agent::Agent;

pub struct Connection {
    agent: Agent,
    desired_version: Protocol,
}

impl Connection {
    pub fn new(agent: Agent, desired_version: Protocol) -> Self {
        Self {
            agent,
            desired_version,
        }
    }
}
