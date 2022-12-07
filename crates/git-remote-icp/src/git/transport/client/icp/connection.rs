use git::protocol::transport;
use git_repository as git;
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::{Agent, Identity};
use log::trace;
use std::sync::Arc;

pub struct Connection {
    pub agent: Agent,
    pub desired_version: transport::Protocol,
}

impl Connection {
    pub fn new(
        identity: Arc<dyn Identity>,
        network_url: &str,
        url: git::Url,
        desired_version: transport::Protocol,
    ) -> Result<Self, transport::connect::Error> {
        trace!("identity: {:#?}", identity);
        trace!("network_url: {:#?}", network_url);
        trace!("url: {:#?}", url);
        trace!("desired_version: {:#?}", desired_version);

        let http_transport = ReqwestHttpReplicaV2Transport::create(network_url)
            .map_err(|err| transport::connect::Error::Connection(Box::new(err)))?;

        let agent = Agent::builder()
            // .with_transport(http_transport)
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
