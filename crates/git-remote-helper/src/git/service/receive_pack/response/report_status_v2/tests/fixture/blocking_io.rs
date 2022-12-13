use super::Fixture;
use git_repository as git;
use git::bstr::{BStr, ByteSlice};
use git::protocol::transport::packetline;

impl<'a> std::io::Read for Fixture<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<'a> std::io::BufRead for Fixture<'a> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}

impl<'a> git::protocol::transport::client::ReadlineBufRead for Fixture<'a> {
    fn readline(
        &mut self,
    ) -> Option<
        std::io::Result<Result<packetline::PacketLineRef<'_>, packetline::decode::Error>>,
    > {
        let bytes: &BStr = self.0.into();
        let mut lines = bytes.lines();
        let res = lines.next()?;
        self.0 = lines.as_bytes();
        Some(Ok(Ok(packetline::PacketLineRef::Data(res))))
    }
}
