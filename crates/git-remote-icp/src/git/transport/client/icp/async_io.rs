use crate::git::transport::client::icp;
use async_trait::async_trait;
use candid::{Decode, Encode};
use git::protocol::futures_lite::io::Cursor;
use git::protocol::futures_lite::AsyncReadExt;
use git::protocol::transport::{client, Protocol, Service};
use git_repository as git;
use ic_certified_assets::types::{HttpRequest, HttpResponse};
use serde_bytes::ByteBuf;

use log::trace;

impl client::TransportWithoutIO for icp::Connection {
    fn request(
        &mut self,
        write_mode: client::WriteMode,
        on_into_read: client::MessageKind,
    ) -> Result<client::RequestWriter<'_>, client::Error> {
        todo!("TransportWithoutIO::request")
    }

    fn to_url(&self) -> std::borrow::Cow<'_, bstr::BStr> {
        todo!("TransportWithoutIO::to_url")
    }

    fn connection_persists_across_multiple_requests(&self) -> bool {
        todo!("TransportWithoutIO::connection_persists_across_multiple_requests")
    }

    fn configure(
        &mut self,
        config: &dyn std::any::Any,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        todo!("TransportWithoutIO::configure")
    }
}

// NOTE: using client::Error::io isn't ideal but seems to be the best option
// given what's available.
#[async_trait(?Send)]
impl client::Transport for icp::Connection {
    async fn handshake<'a>(
        &mut self,
        service: Service,
        // TODO: use these
        extra_parameters: &'a [(&'a str, Option<&'a str>)],
    ) -> Result<client::SetServiceResponse<'_>, client::Error> {
        trace!("service: {:#?}", service);
        trace!("extra_parameters: {:#?}", extra_parameters);

        let host_header = self.url.host().map(|host| {
            let host = match self.url.port {
                Some(port) => format!("{}:{}", host, port),
                None => host.to_string(),
            };
            ("host".to_string(), host)
        });

        let git_protocol_header = {
            let version = match self.desired_version {
                Protocol::V1 => "1",
                Protocol::V2 => "2",
            };
            ("git-protocol".to_string(), format!("version={}", version))
        };

        // Calling HTTP methods for now instead of exposing separate methods for
        // each service.
        //
        // This is currently an artifact of intially using HTTP as the
        // transport, but it has the benefit of keeping the existing HTTP
        // interface working for unauthenticated calls that don't use the remote
        // helper.
        let result = match service {
            Service::ReceivePack => {
                /*
                self.agent
                    .update(&self.canister_id, "http_request_update")
                    // .with_arg(&http_request)
                    .call_and_wait()
                    .await
                */
                todo!("Transport::handshake Service::ReceivePack")
            }
            Service::UploadPack => {
                // url: "/@paul/hello-world.git/info/refs?service=git-upload-pack".to_string(),
                let url = format!("{}/info/refs?service=git-upload-pack", self.url.path);

                let headers = vec![host_header, Some(git_protocol_header)]
                    .into_iter()
                    .filter_map(|x| x)
                    .collect::<Vec<_>>();

                let http_request = HttpRequest {
                    method: "GET".to_string(),
                    url,
                    headers,
                    // body: ByteBuf::from(body),
                    body: ByteBuf::default(),
                };

                trace!("http_request: {:#?}", http_request);

                let arg =
                    candid::Encode!(&http_request).map_err(|candid_error| client::Error::Io {
                        err: std::io::Error::new(std::io::ErrorKind::Other, candid_error),
                    })?;

                // TODO: consider using query_signed or update here
                self.agent
                    .query(&self.canister_id, "http_request")
                    .with_arg(&arg)
                    .call()
                    .await
            }
        };

        let response = result.map_err(|agent_error| {
            // TODO: consider mapping AgentError::HttpError to client::Error::Http
            client::Error::Io {
                err: std::io::Error::new(std::io::ErrorKind::Other, agent_error),
            }
        })?;

        let response = Decode!(response.as_slice(), HttpResponse).map_err(|candid_error| {
            client::Error::Io {
                err: std::io::Error::new(std::io::ErrorKind::Other, candid_error),
            }
        })?;

        // TODO: consider mapping HttpResponse to client::Error::Http

        trace!("response: {:#?}", response);
        trace!("response.body: {}", String::from_utf8_lossy(&response.body));

        use git::protocol::transport::packetline::{PacketLineRef, StreamingPeekableIter};

        let line_reader = self.line_provider.get_or_insert_with(|| {
            let async_reader = Cursor::new(response.body.to_vec());
            StreamingPeekableIter::new(async_reader, &[PacketLineRef::Flush])
        });

        // The service announcement is only sent sometimes depending on the
        // exact server/protocol version/used protocol (http?) eat the
        // announcement when its there to avoid errors later (and check that the
        // correct service was announced). Ignore the announcement otherwise.
        let line_ = line_reader
            .peek_line()
            .await
            .ok_or(client::Error::ExpectedLine(
                "capabilities, version or service",
            ))???;

        let line = line_.as_text().ok_or(client::Error::ExpectedLine("text"))?;

        if let Some(announced_service) = line.as_bstr().strip_prefix(b"# service=") {
            if announced_service != service.as_str().as_bytes() {
                return Err(client::Error::Io {
                    err: std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Expected to see service {:?}, but got {:?}",
                            service.as_str(),
                            announced_service
                        ),
                    ),
                });
            }

            line_reader.as_read().read_to_end(&mut Vec::new()).await?;
        }

        let client::capabilities::recv::Outcome {
            capabilities,
            refs,
            protocol: actual_protocol,
        } = client::Capabilities::from_lines_with_version_detection(line_reader).await?;

        trace!("capabilities: {:#?}", capabilities);
        // trace!("refs: {:#?}", refs);
        trace!("actual_protocol: {:#?}", actual_protocol);

        Ok(client::SetServiceResponse {
            actual_protocol,
            capabilities,
            refs,
        })
    }
}
