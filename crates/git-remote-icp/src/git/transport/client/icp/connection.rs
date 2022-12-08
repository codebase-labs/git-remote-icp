use git::protocol::transport;
use git_repository as git;
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::export::Principal;
use ic_agent::{Agent, Identity};
use log::trace;
use std::sync::Arc;

pub struct Connection {
    pub agent: Agent,
    pub replica_url: String,
    pub canister_id: Principal,
    pub desired_version: transport::Protocol,
}

impl Connection {
    pub fn new(
        identity: Arc<dyn Identity>,
        replica_url: &str,
        canister_id: Principal,
        url: git::Url,
        desired_version: transport::Protocol,
    ) -> Result<Self, transport::connect::Error> {
        trace!("Connection::new");
        trace!("identity: {:#?}", identity);
        trace!("replica_url: {}", replica_url);
        trace!("canister_id: {}", canister_id);
        trace!("url: {:#?}", url);
        trace!("desired_version: {:#?}", desired_version);

        let replica_transport = ReqwestHttpReplicaV2Transport::create(replica_url)
            .map_err(|err| transport::connect::Error::Connection(Box::new(err)))?;

        let agent = Agent::builder()
            .with_transport(replica_transport)
            .with_arc_identity(identity.clone())
            .build()
            .map_err(|err| transport::connect::Error::Connection(Box::new(err)))?;

        let connection = Self {
            agent,
            replica_url: replica_url.to_string(),
            canister_id,
            desired_version,
        };

        Ok(connection)
    }
}
