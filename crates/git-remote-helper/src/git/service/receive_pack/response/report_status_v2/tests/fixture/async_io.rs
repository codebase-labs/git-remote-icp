use super::Fixture;
use async_trait::async_trait;
use core::pin::Pin;
use git::bstr::{BStr, ByteSlice};
use git::protocol::transport::packetline;
use git_repository as git;

impl<'a> Fixture<'a> {
    fn project(self: Pin<&mut Self>) -> Pin<&mut &'a [u8]> {
        unsafe { Pin::new(&mut self.get_unchecked_mut().0) }
    }
}

impl<'a> git::protocol::futures_io::AsyncRead for Fixture<'a> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.project().poll_read(cx, buf)
    }
}

impl<'a> git::protocol::futures_io::AsyncBufRead for Fixture<'a> {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        self.project().poll_fill_buf(cx)
    }

    fn consume(self: std::pin::Pin<&mut Self>, amt: usize) {
        self.project().consume(amt)
    }
}

#[async_trait(?Send)]
impl<'a> git::protocol::transport::client::ReadlineBufRead for Fixture<'a> {
    async fn readline(
        &mut self,
    ) -> Option<std::io::Result<Result<packetline::PacketLineRef<'_>, packetline::decode::Error>>>
    {
        let bytes: &BStr = self.0.into();
        let mut lines = bytes.lines();
        let res = lines.next()?;
        self.0 = lines.as_bytes();
        Some(Ok(Ok(packetline::PacketLineRef::Data(res))))
    }
}
