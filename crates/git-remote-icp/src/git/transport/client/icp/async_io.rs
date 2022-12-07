use crate::git::transport::client::icp;
use async_trait::async_trait;
use git::protocol::transport::client;
use git_repository as git;

impl client::TransportWithoutIO for icp::Connection {
    fn request(
        &mut self,
        write_mode: client::WriteMode,
        on_into_read: client::MessageKind,
    ) -> Result<client::RequestWriter<'_>, client::Error> {
        todo!()
    }

    fn to_url(&self) -> std::borrow::Cow<'_, bstr::BStr> {
        todo!()
    }

    fn connection_persists_across_multiple_requests(&self) -> bool {
        todo!()
    }

    fn configure(
        &mut self,
        config: &dyn std::any::Any,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        todo!()
    }
}

#[async_trait(?Send)]
impl client::Transport for icp::Connection {
    async fn handshake<'a>(
        &mut self,
        service: git::protocol::transport::Service,
        extra_parameters: &'a [(&'a str, Option<&'a str>)],
    ) -> Result<client::SetServiceResponse<'_>, client::Error> {
        todo!()
    }
}
