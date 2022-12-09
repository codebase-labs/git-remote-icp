// This calls HTTP methods for now instead of exposing separate methods for each
// service.
//
// This is currently an artifact of intially using HTTP as the transport, but it
// has the benefit of keeping the existing HTTP interface working for
// unauthenticated calls that don't use the remote helper.

use crate::git::transport::client::icp;
use async_trait::async_trait;
use candid::{Decode, Encode};
use git::protocol::futures_lite::future;
use git::protocol::futures_lite::io::Cursor;
use git::protocol::futures_lite::AsyncReadExt;
use git::protocol::transport::packetline::{PacketLineRef, StreamingPeekableIter};
use git::protocol::transport::{client, Protocol, Service};
use git_repository as git;
use ic_certified_assets::types::{HeaderField, HttpRequest, HttpResponse};
use log::trace;
use serde_bytes::ByteBuf;

fn append_url(base: &str, suffix: &str) -> String {
    let mut buf = base.to_owned();
    if base.as_bytes().last() != Some(&b'/') {
        buf.push('/');
    }
    buf.push_str(suffix);
    buf
}

enum Action {
    Handshake,
    Request,
}

impl icp::Connection {
    async fn call(
        &self,
        action: Action,
        service: Service,
        url: String,
        static_headers: &[HeaderField],
        dynamic_headers: &mut Vec<HeaderField>,
        body: ByteBuf,
    ) -> Result<(Vec<HeaderField>, Cursor<Vec<u8>>), client::Error> {
        if let Some(host) = self.url.host() {
            let host = match self.url.port {
                Some(port) => format!("{}:{}", host, port),
                None => host.to_string(),
            };
            dynamic_headers.push(("host".to_string(), host))
        }

        let headers = static_headers
            .iter()
            .chain(&*dynamic_headers)
            .map(|x| x.to_owned())
            .collect::<Vec<_>>();

        let method = match action {
            Action::Handshake => "GET",
            Action::Request => "POST",
        }
        .to_string();

        let http_request = HttpRequest {
            method,
            url,
            headers,
            body,
        };

        trace!("http_request: {:#?}", http_request);

        let arg = candid::Encode!(&http_request).map_err(|candid_error| client::Error::Io {
            err: std::io::Error::new(std::io::ErrorKind::Other, candid_error),
        })?;

        // TODO
        // `.call(&self.canister_id, "http_request")` with `.call()` for handshake
        // `.update(&self.canister_id, "http_request_update")` with `.call_and_wait()` for request

        // TODO: consider using query_signed, or update even if query works
        let result = self
            .agent
            // .query(&self.canister_id, "http_request_update")
            .update(&self.canister_id, "http_request_update")
            .with_arg(&arg)
            // .call()
            .call_and_wait()
            .await;

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

        let response_body = Cursor::new(response.body.to_vec());
        return Ok((response.headers, response_body));
    }

    fn check_content_type(
        service: Service,
        kind: &str,
        headers: Vec<HeaderField>,
    ) -> Result<(), client::Error> {
        let wanted_content_type = format!("application/x-{}-{}", service.as_str(), kind);

        if !headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("content-type") && value.trim() == wanted_content_type
        }) {
            return Err(client::Error::Io {
                err: std::io::Error::new(std::io::ErrorKind::Other, format!(
                    "Didn't find '{}' header to indicate 'smart' protocol, and 'dumb' protocol is not supported.",
                    wanted_content_type
                )),
            });
        }
        Ok(())
    }
}

impl client::TransportWithoutIO for icp::Connection {
    fn request(
        &mut self,
        write_mode: client::WriteMode,
        on_into_read: client::MessageKind,
    ) -> Result<client::RequestWriter<'_>, client::Error> {
        let service = self
            .service
            .expect("handshake() must have been called first");

        trace!("service: {:#?}", service);

        let url = append_url(&self.url.path.to_string(), service.as_str());

        let static_headers = &[
            self.user_agent_header.clone(),
            (
                "Content-Type".to_string(),
                format!("application/x-{}-request", service.as_str()),
            ),
            (
                "Accept".to_string(),
                format!("application/x-{}-result", service.as_str()),
            ),
            // Needed to avoid sending Expect: 100-continue, which adds another
            // response and only CURL wants that
            ("Expect".to_string(), "".to_string()),
        ];

        let mut dynamic_headers = Vec::new();

        if self.actual_version != Protocol::V1 {
            dynamic_headers.push((
                "Git-Protocol".to_string(),
                format!("version={}", self.actual_version as usize),
            ));
        }

        let (_response_headers, response_body) = future::block_on(self.call(
            Action::Request,
            service,
            url,
            static_headers,
            &mut dynamic_headers,
            // FIXME
            ByteBuf::new(),
        ))?;

        let line_provider = self
            .line_provider
            .as_mut()
            .expect("handshake to have been called first");

        line_provider.replace(response_body);

        todo!("TransportWithoutIO::request")

        /*
        let writer = body;

        let reader = Box::new(HeadersThenBody::<H, _> {
            service,
            headers: Some(headers),
            body: line_provider.as_read_without_sidebands(),
        });

        Ok(client::RequestWriter::new_from_bufread(
            writer,
            reader,
            write_mode,
            on_into_read,
        ))
         */
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
        extra_parameters: &'a [(&'a str, Option<&'a str>)],
    ) -> Result<client::SetServiceResponse<'_>, client::Error> {
        trace!("service: {:#?}", service);
        trace!("extra_parameters: {:#?}", extra_parameters);

        let url = append_url(
            &self.url.path.to_string(),
            &format!("info/refs?service={}", service.as_str()),
        );

        let static_headers = &[self.user_agent_header.clone()];

        let mut dynamic_headers = Vec::<HeaderField>::new();

        if self.desired_version != Protocol::V1 || !extra_parameters.is_empty() {
            let mut parameters = if self.desired_version != Protocol::V1 {
                let mut p = format!("version={}", self.desired_version as usize);
                if !extra_parameters.is_empty() {
                    p.push(':');
                }
                p
            } else {
                String::new()
            };

            parameters.push_str(
                &extra_parameters
                    .iter()
                    .map(|(key, value)| match value {
                        Some(value) => format!("{}={}", key, value),
                        None => key.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(":"),
            );

            dynamic_headers.push(("Git-Protocol".to_string(), parameters));
        }

        let (response_headers, response_body) = self
            .call(
                Action::Handshake,
                service,
                url,
                static_headers,
                &mut dynamic_headers,
                ByteBuf::new(),
            )
            .await?;

        icp::Connection::check_content_type(service, "advertisement", response_headers)?;

        let line_reader = self.line_provider.get_or_insert_with(|| {
            StreamingPeekableIter::new(response_body, &[PacketLineRef::Flush])
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
        trace!("actual_protocol: {:#?}", actual_protocol);

        self.actual_version = actual_protocol;
        self.service = Some(service);

        Ok(client::SetServiceResponse {
            actual_protocol,
            capabilities,
            refs,
        })
    }
}
