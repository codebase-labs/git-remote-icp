use git::protocol::futures_lite::io::Cursor;
use git::protocol::transport;
use git_repository as git;
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::export::Principal;
use ic_agent::{Agent, Identity};
use ic_certified_assets::types::HeaderField;
use log::trace;
use std::sync::Arc;
use transport::packetline::StreamingPeekableIter;

pub struct Connection {
    pub line_provider: Option<StreamingPeekableIter<Cursor<Vec<u8>>>>,
    pub agent: Agent,
    pub replica_url: String,
    pub canister_id: Principal,
    pub url: git::Url,
    pub user_agent_header: HeaderField,
    // pub supported_versions: [Protocol; 1],
    pub desired_version: transport::Protocol,
    pub actual_version: transport::Protocol,
    pub service: Option<transport::Service>,
}

impl Connection {
    pub async fn new(
        identity: Arc<dyn Identity>,
        fetch_root_key: bool,
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

        // TODO: icp.fetchRootKey
        if fetch_root_key {
            agent
                .fetch_root_key()
                .await
                .map_err(|err| transport::connect::Error::Connection(Box::new(err)))?;
        }

        let connection = Self {
            line_provider: None,
            agent,
            replica_url: replica_url.to_string(),
            canister_id,
            url,
            user_agent_header: (
                "User-Agent".to_string(),
                concat!("git/remote-icp-", env!("CARGO_PKG_VERSION")).to_string(),
            ),
            // TODO: Protocol::V2?
            // supported_versions: [desired_version],
            desired_version,
            // TODO: None?
            actual_version: desired_version,
            service: None,
        };

        Ok(connection)
    }
}
