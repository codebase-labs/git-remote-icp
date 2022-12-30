// Based on
// https://github.com/Byron/gitoxide/blob/e6b9906c486b11057936da16ed6e0ec450a0fb83/git-transport/src/client/blocking_io/http/reqwest/remote.rs

use crate::{http, http::reqwest::Remote};

use candid::{Decode, Encode};
use git_features::io::pipe;
use git_repository as git;
use ic_agent::export::Principal;
use ic_agent::Agent;
use ic_certified_assets::types::{HttpRequest, HttpResponse};
use log::trace;
use serde_bytes::ByteBuf;
use std::any::Any;
use std::io::{Read, Write};
use std::ops::Deref;
use tokio::runtime::Runtime;

/// The error returned by the 'remote' helper, a purely internal construct to perform http requests.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

impl git::protocol::transport::IsSpuriousError for Error {
    fn is_spurious(&self) -> bool {
        match self {
            Error::Reqwest(err) => {
                err.is_timeout()
                    || err.is_connect()
                    || err
                        .status()
                        .map_or(false, |status| status.is_server_error())
            }
        }
    }
}

impl Remote {
    pub fn new(agent: Agent, canister_id: Principal) -> Self {
        let (req_send, req_recv) = std::sync::mpsc::sync_channel(0);
        let (res_send, res_recv) = std::sync::mpsc::sync_channel(0);
        let runtime = Runtime::new().expect("failed to create runtime");
        let moved_agent = agent.clone();
        let handle = std::thread::spawn(move || -> Result<(), Error> {
            // We may error while configuring, which is expected as part of the internal protocol. The error will be
            // received and the sender of the request might restart us.
            for Request {
                url,
                headers,
                upload,
            } in req_recv
            {
                let (post_body_tx, mut post_body_rx) = pipe::unidirectional(0);
                let (mut response_body_tx, response_body_rx) = pipe::unidirectional(0);
                let (mut headers_tx, headers_rx) = pipe::unidirectional(0);

                if res_send
                    .send(Response {
                        headers: headers_rx,
                        body: response_body_rx,
                        upload_body: post_body_tx,
                    })
                    .is_err()
                {
                    // This means our internal protocol is violated as the one who sent the request isn't listening anymore.
                    // Shut down as something is off.
                    break;
                }

                let mut body = ByteBuf::new();

                if upload {
                    if let Err(err) = post_body_rx.read_to_end(&mut body) {
                        let kind = std::io::ErrorKind::Other;
                        let err = Err(std::io::Error::new(kind, err));
                        response_body_tx.channel.send(err).ok();
                        continue;
                    }
                }

                let method = if upload { "POST" } else { "GET" }.to_string();

                let http_request = HttpRequest {
                    method,
                    url,
                    headers,
                    body,
                };

                trace!("http_request: {:#?}", http_request);

                let arg = match candid::Encode!(&http_request) {
                    Ok(arg) => arg,
                    Err(err) => {
                        let kind = std::io::ErrorKind::Other;
                        let err = Err(std::io::Error::new(kind, err));
                        headers_tx.channel.send(err).ok();
                        continue;
                    }
                };

                let res = if upload {
                    runtime.block_on(
                        moved_agent
                            .update(&canister_id, "http_request_update")
                            .with_arg(&arg)
                            .call_and_wait(),
                    )
                } else {
                    runtime.block_on(
                        moved_agent
                            .query(&canister_id, "http_request")
                            .with_arg(&arg)
                            .call(),
                    )
                };

                let res = res
                    .map_err(|agent_error| {
                        std::io::Error::new(std::io::ErrorKind::Other, agent_error)
                    })
                    .and_then(|res| {
                        Decode!(res.as_slice(), HttpResponse).map_err(|candid_error| {
                            std::io::Error::new(std::io::ErrorKind::Other, candid_error)
                        })
                    })
                    .and_then(|res| match res.status_code {
                        400..=499 | 500..=599 => reqwest::StatusCode::from_u16(res.status_code)
                            .map_err(|invalid_status_code_error| {
                                std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    invalid_status_code_error,
                                )
                            })
                            .and_then(|status| {
                                let kind = if status == reqwest::StatusCode::UNAUTHORIZED {
                                    std::io::ErrorKind::PermissionDenied
                                } else if status.is_server_error() {
                                    std::io::ErrorKind::ConnectionAborted
                                } else {
                                    std::io::ErrorKind::Other
                                };
                                let err = format!("Received HTTP status {}", status.as_str());
                                Err(std::io::Error::new(kind, err))
                            }),
                        _ => Ok(res),
                    });

                let res = match res {
                    Ok(res) => res,
                    Err(err) => {
                        headers_tx.channel.send(Err(err)).ok();
                        continue;
                    }
                };

                let send_headers = {
                    move || -> std::io::Result<()> {
                        for (name, value) in res.headers {
                            headers_tx.write_all(name.as_str().as_bytes())?;
                            headers_tx.write_all(b":")?;
                            headers_tx.write_all(value.as_bytes())?;
                            headers_tx.write_all(b"\n")?;
                        }
                        // Make sure this is an FnOnce closure to signal the remote reader we are done.
                        drop(headers_tx);
                        Ok(())
                    }
                };

                // We don't have to care if anybody is receiving the header, as
                // a matter of fact we cannot fail sending them. Thus an error
                // means the receiver failed somehow, but might also have
                // decided not to read headers at all. Fine with us.
                send_headers().ok();

                // Reading the response body is streaming and may fail for many
                // reasons. If so, we send the error over the response body
                // channel and that's all we can do.
                if let Err(err) = std::io::copy(&mut res.body.deref(), &mut response_body_tx) {
                    response_body_tx.channel.send(Err(err)).ok();
                }
            }
            Ok(())
        });

        Remote {
            agent,
            canister_id,
            handle: Some(handle),
            request: req_send,
            response: res_recv,
        }
    }
}

/// utilities
impl Remote {
    fn make_request(
        &mut self,
        url: &str,
        _base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
        upload: bool,
    ) -> Result<http::PostResponse<pipe::Reader, pipe::Reader, pipe::Writer>, http::Error> {
        let mut header_values = Vec::new();
        for header_line in headers {
            let header_line = header_line.as_ref();
            let colon_pos = header_line
                .find(':')
                .expect("header line must contain a colon to separate key and value");
            let header_name = &header_line[..colon_pos];
            let value = &header_line[colon_pos + 1..];
            header_values.push((header_name.to_string(), value.to_string()));
        }
        self.request
            .send(Request {
                url: url.to_owned(),
                headers: header_values,
                upload,
            })
            .expect("the remote cannot be down at this point");

        let Response {
            headers,
            body,
            upload_body,
        } = match self.response.recv() {
            Ok(res) => res,
            Err(_) => {
                let err = self
                    .handle
                    .take()
                    .expect("always present")
                    .join()
                    .expect("no panic")
                    .expect_err("no receiver means thread is down with init error");
                *self = Self::new(self.agent.clone(), self.canister_id);
                return Err(http::Error::InitHttpClient {
                    source: Box::new(err),
                });
            }
        };

        Ok(http::PostResponse {
            post_body: upload_body,
            headers,
            body,
        })
    }
}

impl http::Http for Remote {
    type Headers = pipe::Reader;
    type ResponseBody = pipe::Reader;
    type PostBody = pipe::Writer;

    fn get(
        &mut self,
        url: &str,
        base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<http::GetResponse<Self::Headers, Self::ResponseBody>, http::Error> {
        self.make_request(url, base_url, headers, false)
            .map(Into::into)
    }

    fn post(
        &mut self,
        url: &str,
        base_url: &str,
        headers: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<http::PostResponse<Self::Headers, Self::ResponseBody, Self::PostBody>, http::Error>
    {
        self.make_request(url, base_url, headers, true)
    }

    fn configure(
        &mut self,
        _config: &dyn Any,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(())
    }
}

pub(crate) struct Request {
    pub url: String,
    pub headers: Vec<ic_certified_assets::types::HeaderField>,
    pub upload: bool,
}

/// A link to a thread who provides data for the contained readers.
/// The expected order is:
/// - write `upload_body`
/// - read `headers` to end
/// - read `body` to end
pub(crate) struct Response {
    pub headers: pipe::Reader,
    pub body: pipe::Reader,
    pub upload_body: pipe::Writer,
}
