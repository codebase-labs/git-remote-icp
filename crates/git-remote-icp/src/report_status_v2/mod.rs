// use anyhow::{anyhow, Context};
// use git::bstr::{BStr, BString, ByteSlice as _, B};
// use git::protocol::futures_io::AsyncRead;
use git::protocol::futures_lite::AsyncBufReadExt as _;
// use git::protocol::futures_lite::AsyncReadExt as _;
// use git::protocol::transport::packetline;
use git::bstr::{BString, B};
use git::protocol::transport::client::ExtendedBufRead;
use git_repository as git;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::char;
use nom::combinator::{eof, opt};
// use nom::error::context;
use nom::IResult;

pub type ReportStatusV2 = (UnpackResult, Vec<CommandStatusV2>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnpackResult {
    Ok,
    ErrorMsg(ErrorMsg),
}

pub enum CommandStatusV2 {
    Ok(RefName, Option<OptionLine>),
    Fail(RefName, ErrorMsg),
}

pub enum OptionLine {
    OptionRefName(RefName),
    OptionOldOid(git::hash::ObjectId),
    OptionNewOid(git::hash::ObjectId),
    OptionForce,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorMsg(BString);

pub struct RefName(BString);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError<I> {
    ErrorMsgIsOk,
    FailedToReadUnpackStatus,
    Io(String),
    Nom(I, nom::error::ErrorKind),
    PacketLineDecode(String),
    UnexpectedFlush,
    UnexpectedDelimiter,
    UnexpectedResponseEnd,
}

impl<I> std::fmt::Display for ParseError<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::ErrorMsgIsOk => "error msg is \"ok\"".to_string(),
            Self::FailedToReadUnpackStatus => "failed to read unpack status".to_string(),
            Self::Io(err) => format!("IO error: {}", err),
            Self::Nom(_input, error_kind) => format!("nom error: {}", error_kind.description()),
            Self::PacketLineDecode(err) => err.to_string(),
            Self::UnexpectedFlush => "unexpected flush packet".to_string(),
            Self::UnexpectedDelimiter => "unexpected delimiter".to_string(),
            Self::UnexpectedResponseEnd => "unexpected response end".to_string(),
        };
        write!(f, "{}", msg)
    }
}

impl<I> std::error::Error for ParseError<I> where I: std::fmt::Debug {}

impl<I> nom::error::ParseError<I> for ParseError<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        ParseError::Nom(input, kind)
    }

    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

pub async fn parse<'a>(
    input: &mut (dyn ExtendedBufRead + Unpin + 'a),
) -> anyhow::Result<ReportStatusV2> {
    let mut buf = String::new();

    // TODO: consider input.fail_on_err_lines(true);
    let _bytes_read = input.read_line(&mut buf).await?;

    let (_next_input, unpack_result) = parse_unpack_status::<'_, ()>(&buf.into_bytes())?;

    // TODO: confirm that we don't need to call buf.clear() because into_bytes() consumes the buf

    // TODO: parse the next line also using read_line with the command_status_v2 parser
    // TODO: parse the remaining lines in a loop with the command_status_v2 parser

    /*
    while let Some(line) = iter.read_line().await {
        let line = line
            .map_err(|_| {
                // FIXME: IoError(std::io::Error)
                nom::Err::Failure(ParseError::FailedToReadUnpackStatus)
            })?
            .map_err(|_| {
                // FIXME: PacketLineDecodeError(packetline::decode::Error)
                nom::Err::Failure(ParseError::FailedToReadUnpackStatus)
            })?;
        // let line = line.as_slice()
        log::debug!("line: {:#?}", line.as_bstr());
    }
    */

    // TODO: .stopped_at() == Some(packetline::PacketLineRef::Flush)

    // TEMP
    let command_statuses = Vec::new();

    // TODO: let refname = git_validate::reference::name()?;

    /*
    let unpack_status = match iter.read_line().await {
        Some(line) => {
            let line = line
               .map_err(|err| nom::Err::Failure(ParseError::Io(err.to_string())))?
                .map_err(|err| nom::Err::Failure(ParseError::PacketLineDecode(err.to_string())))?;

            // Similar to line.as_slice() but with a custom error
            let line = match line {
                packetline::PacketLineRef::Data(data) => Ok(data),
                packetline::PacketLineRef::Flush => {
                    Err(nom::Err::Failure(ParseError::UnexpectedFlush))
                }
                packetline::PacketLineRef::Delimiter => {
                    Err(nom::Err::Failure(ParseError::UnexpectedDelimiter))
                }
                packetline::PacketLineRef::ResponseEnd => {
                    Err(nom::Err::Failure(ParseError::UnexpectedResponseEnd))
                }
            }?;

            parse_unpack_status(line)
            // Err(nom::Err::Failure(ParseError::FailedToReadUnpackStatus))
        }
        None => Err(nom::Err::Failure(ParseError::FailedToReadUnpackStatus)),
    }?;
    */

    Ok((unpack_result, command_statuses))
}

fn parse_unpack_status<'a, Error: nom::error::ParseError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], UnpackResult, Error> {
    let (next_input, _unpack) = tag(b"unpack")(input)?;
    let (next_input, _space) = char(' ')(next_input)?;
    let (next_input, unpack_result) = parse_unpack_result(next_input)?;
    let (next_input, _newline) = opt(char('\n'))(next_input)?;
    let (next_input, _) = eof(next_input)?;
    Ok((next_input, unpack_result))
}

fn parse_unpack_result<'a, Error: nom::error::ParseError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], UnpackResult, Error> {
    alt((
        nom::combinator::map(tag(b"ok"), |_| UnpackResult::Ok),
        nom::combinator::map(parse_error_msg, UnpackResult::ErrorMsg),
    ))(input)
}

// TODO: send commit without tree to trigger error for test case
/*
fn parse_error_msg<'a, Error: nom::error::ParseError<&'a [u8]>>(
    input: &'a [u8],
) -> IResult<&'a [u8], ErrorMsg, Error> {
    // FIXME: this should be 1*(OCTET)
    let (next_input, error_msg) =
        nom::combinator::verify(nom::combinator::rest, |bytes: &[u8]| bytes != b"ok")(input)?;

    Ok((next_input, ErrorMsg(BString::from(error_msg))))
}
*/

fn parse_error_msg<'a, Error: nom::error::ParseError<&'a [u8]>>(
    input: &'a [u8]
) -> IResult<&'a [u8], ErrorMsg, Error> {
    todo!()
    // if input != B("ok") {
    //     let (next_input, error_msg) = take_while1(is_octet)(input)?;
    //     Ok((next_input, ErrorMsg(BString::from(error_msg))))
    // } else {
    //     Err(nom::Err::Failure(ParseError::ErrorMsgIsOk))
    // }
}

fn is_octet(chr: u8) -> bool {
    chr >= 0x00 && chr <= 0xFF
    // true
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_parse_unpack_status_ok() {
//         let input = b"unpack ok";
//         let result = parse_unpack_status(input);
//         assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok");
//     }

//     #[test]
//     fn test_parse_unpack_status_ok_newline() {
//         let input = b"unpack ok\n";
//         let result = parse_unpack_status(input);
//         assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok");
//     }

//     #[test]
//     fn test_parse_unpack_status_error_msg() {
//         let input = b"unpack some error message";
//         let result = parse_unpack_status(input);
//         assert_eq!(
//             result.map(|x| x.1),
//             Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
//                 b"some error message".to_vec()
//             )))),
//             "error msg"
//         );
//     }

//     #[test]
//     fn test_parse_unpack_status_error_msg_newline() {
//         let input = b"unpack some error message\n";
//         let result = parse_unpack_status(input);
//         assert_eq!(
//             result.map(|x| x.1),
//             Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
//                 b"some error message\n".to_vec()
//             )))),
//             "error msg"
//         );
//     }

//     #[test]
//     fn test_parse_unpack_result_ok() {
//         let input = b"ok";
//         let result = parse_unpack_result(input);
//         assert_eq!(result.map(|x| x.1), Ok(UnpackResult::Ok), "ok");
//     }

//     #[test]
//     fn test_parse_unpack_result_error_msg() {
//         let input = b"some error message";
//         let result = parse_unpack_result(input);
//         assert_eq!(
//             result.map(|x| x.1),
//             Ok(UnpackResult::ErrorMsg(ErrorMsg(BString::new(
//                 input.to_vec()
//             )))),
//             "error msg"
//         );
//     }

//     #[test]
//     fn test_parse_error_msg_not_ok() {
//         let input = b"some error message";
//         let result = parse_error_msg(input);
//         assert_eq!(
//             result.map(|x| x.1),
//             Ok(ErrorMsg(BString::new(input.to_vec()))),
//             "error msg not ok"
//         );
//     }

//     #[test]
//     fn test_parse_error_msg_ok() {
//         let input = b"ok";
//         let result = parse_error_msg(input);
//         assert_eq!(
//             result,
//             Err(nom::Err::Failure(ParseError::ErrorMsgIsOk)),
//             "error msg is ok"
//         );
//     }
// }
