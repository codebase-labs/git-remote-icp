mod fixture;

use super::*;
use fixture::Fixture;
use git::bstr::ByteSlice;
use git_repository as git;
use maybe_async::maybe_async;

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_ok_0_command_status_v2() {
    let mut input = vec!["000eunpack ok", "0000"].join("\n").into_bytes();
    let reader = Fixture(&mut input);
    let result = read_and_parse(reader).await;
    assert_eq!(
        result,
        Err(ParseError::ExpectedOneOrMoreCommandStatusV2),
        "report-status-v2"
    )
}

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_ok_1_command_status_v2_ok() {
    let mut input = vec!["000eunpack ok", "0017ok refs/heads/main", "0000"]
        .join("\n")
        .into_bytes();
    let reader = Fixture(&mut input);
    let result = read_and_parse(reader).await;
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

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_ok_1_command_status_v2_fail() {
    let mut input = vec![
        "000eunpack ok",
        "002ang refs/heads/main some error message",
        "0000",
    ]
    .join("\n")
    .into_bytes();
    let reader = Fixture(&mut input);
    let result = read_and_parse(reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![CommandStatusV2::Fail(
                RefName(BString::new(b"refs/heads/main".to_vec())),
                ErrorMsg(BString::new(b"some error message\n".to_vec()))
            ),]
        )),
        "report-status-v2"
    )
}

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_ok_2_command_statuses_v2_ok_fail() {
    let mut input = vec![
        "000eunpack ok",
        "0018ok refs/heads/debug",
        "0028ng refs/heads/main non-fast-forward",
        "0000",
    ]
    .join("\n")
    .into_bytes();
    let reader = Fixture(&mut input);
    let result = read_and_parse(reader).await;
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
                    ErrorMsg(BString::new(b"non-fast-forward\n".to_vec()))
                ),
            ]
        )),
        "report-status-v2"
    )
}

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_ok_2_command_statuses_v2_fail_ok() {
    let mut input = vec![
        "000eunpack ok",
        "0028ng refs/heads/main non-fast-forward",
        "0018ok refs/heads/debug",
        "0000",
    ]
    .join("\n")
    .into_bytes();
    let reader = Fixture(&mut input);
    let result = read_and_parse(reader).await;
    assert_eq!(
        result,
        Ok((
            UnpackResult::Ok,
            vec![
                CommandStatusV2::Fail(
                    RefName(BString::new(b"refs/heads/main".to_vec())),
                    ErrorMsg(BString::new(b"non-fast-forward\n".to_vec()))
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

#[maybe_async]
#[test]
fn test_parse_unpack_status_ok() {
    let input = b"unpack ok";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok")
}

#[maybe_async]
#[test]
fn test_parse_unpack_status_ok_newline() {
    let input = b"unpack ok\n";
    let result = parse_unpack_status::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok")
}

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
#[test]
fn test_parse_unpack_result_ok() {
    let input = b"ok";
    let result = parse_unpack_result::<nom::error::Error<_>>(input);
    assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok");
}

#[maybe_async]
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

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
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

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
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
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_1_option_lines() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_1_option_lines_newline() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_2_option_lines() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_2_option_lines_newline() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_3_option_lines() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_3_option_lines_newline() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_4_option_lines() {
    todo!()
}

#[ignore]
#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
async fn test_read_and_parse_command_status_v2_command_ok_v2_4_option_lines_newline() {
    todo!()
}

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
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

#[maybe_async::test(
    feature = "blocking-network-client",
    async(feature = "async-network-client", tokio::test)
)]
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

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
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

#[maybe_async]
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
