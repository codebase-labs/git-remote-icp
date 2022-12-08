use crate::git::transport::client::icp;
use async_trait::async_trait;
use candid::{Decode, Encode};
use git::protocol::transport::{client, Protocol, Service};
use git_repository as git;
use ic_certified_assets::rc_bytes::RcBytes;
use ic_certified_assets::types::{HttpRequest, HttpResponse};
use serde_bytes::ByteBuf;

use log::trace;

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

// NOTE: using client::Error::io isn't ideal but seems to be the best option
// given what's available.
#[async_trait(?Send)]
impl client::Transport for icp::Connection {
    async fn handshake<'a>(
        &mut self,
        service: Service,
        extra_parameters: &'a [(&'a str, Option<&'a str>)],
    ) -> Result<client::SetServiceResponse<'_>, client::Error> {
        trace!("service: {:#?}", service);
        trace!("extra_parameters: {:#?}", extra_parameters);

        /*
        let result = match service {
            Service::ReceivePack => {
                self.agent.update(&self.canister_id, "receive_pack")
                    // .with_arg(&Encode!(&Argument { })?)
                    .call_and_wait().await
            },
            Service::UploadPack => {
                // TODO: consider using query_signed or update here
                self.agent.query(&self.canister_id, "upload_pack")
                    // .with_arg(&Encode!(&Argument { })?)
                    .call().await
            },
        };

        let response = result.map_err(|agent_error| {
            client::Error::Io {
                err: std::io::Error::new(std::io::ErrorKind::Other, agent_error),
            }
        })?;
        */

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
                todo!()
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

        // trace!("response: {:#?}", response);
        // trace!("response: {:#?}", String::from_utf8_lossy(&response));

        let response = Decode!(response.as_slice(), HttpResponse).map_err(|candid_error| {
            client::Error::Io {
                err: std::io::Error::new(std::io::ErrorKind::Other, candid_error),
            }
        })?;

        // TODO: consider mapping HttpResponse to client::Error::Http

        trace!("response: {:#?}", response);

        todo!()
    }
}
