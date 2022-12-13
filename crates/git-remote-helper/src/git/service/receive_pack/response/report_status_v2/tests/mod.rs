use super::*;
use core::pin::Pin;
use git_repository as git;
use git::bstr::{BStr, ByteSlice};

#[cfg(feature = "async-network-client")]
use async_trait::async_trait;

struct Fixture<'a>(&'a [u8]);

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

#[tokio::test]
async fn test_read_and_parse_ok_0_command_status_v2() {
    let mut input = vec!["unpack ok"].join("\n").into_bytes();
    let mut reader = Fixture(&mut input);
    let result = read_and_parse(&mut reader).await;
    assert_eq!(
        result,
        Err(ParseError::ExpectedOneOrMoreCommandStatusV2),
        "report-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_ok_1_command_status_v2_ok() {
    let mut input = vec!["unpack ok", "ok refs/heads/main"]
        .join("\n")
        .into_bytes();
    let mut reader = Fixture(&mut input);
    let result = read_and_parse(&mut reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![CommandStatusV2::Ok(
                RefName(BString::new(b"refs/heads/main".to_vec())),
                Vec::new(),
            ),]
        )),
        "report-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_ok_1_command_status_v2_fail() {
    let mut input = vec!["unpack ok", "ng refs/heads/main some error message"]
        .join("\n")
        .into_bytes();
    let mut reader = Fixture(&mut input);
    let result = read_and_parse(&mut reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![CommandStatusV2::Fail(
                RefName(BString::new(b"refs/heads/main".to_vec())),
                ErrorMsg(BString::new(b"some error message".to_vec()))
            ),]
        )),
        "report-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_ok_2_command_statuses_v2_ok_fail() {
    let mut input = vec![
        "unpack ok",
        "ok refs/heads/debug",
        "ng refs/heads/main non-fast-forward",
    ]
    .join("\n")
    .into_bytes();
    let mut reader = Fixture(&mut input);
    let result = read_and_parse(&mut reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![
                CommandStatusV2::Ok(
                    RefName(BString::new(b"refs/heads/debug".to_vec())),
                    Vec::new(),
                ),
                CommandStatusV2::Fail(
                    RefName(BString::new(b"refs/heads/main".to_vec())),
                    ErrorMsg(BString::new(b"non-fast-forward".to_vec()))
                ),
            ]
        )),
        "report-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_ok_2_command_statuses_v2_fail_ok() {
    let mut input = vec![
        "unpack ok",
        "ng refs/heads/main non-fast-forward",
        "ok refs/heads/debug",
    ]
    .join("\n")
    .into_bytes();
    let mut reader = Fixture(&mut input);
    let result = read_and_parse(&mut reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![
                CommandStatusV2::Fail(
                    RefName(BString::new(b"refs/heads/main".to_vec())),
                    ErrorMsg(BString::new(b"non-fast-forward".to_vec()))
                ),
                CommandStatusV2::Ok(
                    RefName(BString::new(b"refs/heads/debug".to_vec())),
                    Vec::new(),
                ),
            ]
        )),
        "report-status-v2"
    )
}

#[test]
fn test_parse_unpack_status_ok() {
    let input = b"unpack ok";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok")
}

#[test]
fn test_parse_unpack_status_ok_newline() {
    let input = b"unpack ok\n";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok")
}

#[test]
fn test_parse_unpack_status_error_msg() {
    let input = b"unpack some error message";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
            b"some error message".to_vec()
        )))),
        "error msg"
    )
}

#[test]
fn test_parse_unpack_status_error_msg_newline() {
    let input = b"unpack some error message\n";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
            b"some error message\n".to_vec()
        )))),
        "error msg"
    )
}

#[test]
fn test_parse_unpack_result_ok() {
    let input = b"ok";
    let result = parse_unpack_result::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok");
}

#[test]
fn test_parse_unpack_result_error_msg() {
    let input = b"some error message";
    let result = parse_unpack_result::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
            input.to_vec()
        )))),
        "error msg"
    )
}

#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_0_option_lines() {
    let input = b"ok refs/heads/main";
    let mut reader = Fixture(input);
    let result = read_and_parse_command_statuses_v2::<nom::error::Error<_>>(&mut reader).await;
    assert_eq!(
        result,
        Ok(vec![CommandStatusV2::Ok(
            RefName(BString::new(b"refs/heads/main".to_vec())),
            Vec::new(),
        )]),
        "command-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_0_option_lines_newline() {
    let input = b"ok refs/heads/main\n";
    let mut reader = Fixture(input);
    let result = read_and_parse_command_statuses_v2::<nom::error::Error<_>>(&mut reader).await;
    assert_eq!(
        result,
        Ok(vec![CommandStatusV2::Ok(
            RefName(BString::new(b"refs/heads/main".to_vec())),
            Vec::new(),
        )]),
        "command-status-v2"
    )
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_1_option_lines() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_1_option_lines_newline() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_2_option_lines() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_2_option_lines_newline() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_3_option_lines() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_3_option_lines_newline() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_4_option_lines() {
    todo!()
}

#[ignore]
#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_ok_v2_4_option_lines_newline() {
    todo!()
}

#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_fail() {
    let input = b"ng refs/heads/main some error message";
    let mut reader = Fixture(input);
    let result = read_and_parse_command_statuses_v2::<nom::error::Error<_>>(&mut reader).await;
    assert_eq!(
        result,
        Ok(vec![CommandStatusV2::Fail(
            RefName(BString::new(b"refs/heads/main".to_vec())),
            ErrorMsg(BString::new(b"some error message".to_vec())),
        )]),
        "command-status-v2"
    )
}

#[tokio::test]
async fn test_read_and_parse_command_status_v2_command_fail_newline() {
    let input = b"ng refs/heads/main some error message\n";
    let mut reader = Fixture(input);
    let result = read_and_parse_command_statuses_v2::<nom::error::Error<_>>(&mut reader).await;
    assert_eq!(
        result,
        Ok(vec![CommandStatusV2::Fail(
            RefName(BString::new(b"refs/heads/main".to_vec())),
            ErrorMsg(BString::new(b"some error message".to_vec())),
        )]),
        "command-status-v2"
    )
}

#[test]
fn test_parse_command_ok() {
    let input = b"ok refs/heads/main";
    let result = parse_command_ok::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(RefName(BString::new(b"refs/heads/main".to_vec()))),
        "command-ok"
    )
}

#[test]
fn test_parse_command_ok_newline() {
    let input = b"ok refs/heads/main\n";
    let result = parse_command_ok::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(RefName(BString::new(b"refs/heads/main".to_vec()))),
        "command-ok"
    )
}

#[test]
fn test_parse_command_fail() {
    let input = b"ng refs/heads/main some error message";
    let result = parse_command_fail::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok((
            RefName(BString::new(b"refs/heads/main".to_vec())),
            ErrorMsg(BString::new(b"some error message".to_vec())),
        )),
        "command-fail"
    )
}

#[test]
fn test_parse_command_fail_newline() {
    let input = b"ng refs/heads/main some error message\n";
    let result = parse_command_fail::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok((
            RefName(BString::new(b"refs/heads/main".to_vec())),
            ErrorMsg(BString::new(b"some error message\n".to_vec())),
        )),
        "command-fail"
    )
}

#[test]
fn test_parse_error_msg_not_ok() {
    let input = b"some error message";
    let result = parse_error_msg::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Ok(ErrorMsg(BString::new(input.to_vec()))),
        "error msg not ok"
    )
}

#[test]
fn test_parse_error_msg_ok() {
    let input = b"ok";
    let result = parse_error_msg::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Err(nom::Err::Error(nom::error::Error {
            input: input.as_bytes(),
            code: nom::error::ErrorKind::Verify
        })),
        "error msg is ok"
    )
}

#[test]
fn test_parse_error_msg_empty() {
    let input = b"";
    let result = parse_error_msg::<nom::error::Error<_>>(input);
    assert_eq!(
        result.map(|x| x.1),
        Err(nom::Err::Error(nom::error::Error {
            input: input.as_bytes(),
            code: nom::error::ErrorKind::Verify
        })),
        "error msg is empty"
    )
}

