use git::bstr::BString;
use git::protocol::transport::client::ExtendedBufRead;
use git::protocol::transport::packetline;
use git_repository as git;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::{eof, opt};
use nom::error::context;
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

async fn parse_data_line<'a, Ok, E>(
    input: &'a mut (dyn ExtendedBufRead + Unpin + 'a),
    mut parser: impl FnMut(&'a [u8]) -> IResult<&'a [u8], Ok>,
    readline_none_err: ParseError,
) -> Result<Ok, ParseError>
where
    E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
{
    match input.readline().await {
        Some(line) => {
            let line = line
                .map_err(|err| ParseError::Io(err.to_string()))?
                .map_err(|err| ParseError::PacketLineDecode(err.to_string()))?;

            // Similar to line.as_slice() but with a custom error
            let line = match line {
                packetline::PacketLineRef::Data(data) => Ok(data),
                packetline::PacketLineRef::Flush => Err(ParseError::UnexpectedFlush),
                packetline::PacketLineRef::Delimiter => Err(ParseError::UnexpectedDelimiter),
                packetline::PacketLineRef::ResponseEnd => Err(ParseError::UnexpectedResponseEnd),
            }?;

            parser(line)
                .map(|x| x.1)
                .map_err(|err| ParseError::Nom(err.to_string()))
        }
        None => Err(readline_none_err),
    }
}

pub async fn parse<'a>(
    input: &'a mut (dyn ExtendedBufRead + Unpin + 'a),
) -> Result<ReportStatusV2, ParseError> {
    // TODO: consider input.fail_on_err_lines(true);

    let unpack_result = parse_data_line::<_, nom::error::Error<_>>(
        input,
        parse_unpack_status,
        ParseError::FailedToReadUnpackStatus,
    )
    .await?;

    let first_command_status = parse_data_line::<_, nom::error::Error<_>>(
        input,
        parse_command_status_v2,
        ParseError::FailedToReadCommandStatus,
    )
    .await?;

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

    // TODO: let refname = git_validate::reference::name()?; or use with nom::combinator::verify

    Ok((unpack_result, command_statuses))
}

fn parse_unpack_status<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], UnpackResult, E>
where
    E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
{
    context("unpack-status", |input| {
        let (next_input, _unpack) = tag(b"unpack")(input)?;
        let (next_input, _space) = char(' ')(next_input)?;
        let (next_input, unpack_result) = parse_unpack_result(next_input)?;
        let (next_input, _newline) = opt(char('\n'))(next_input)?;
        let (next_input, _) = eof(next_input)?;
        Ok((next_input, unpack_result))
    })(input)
}

fn parse_unpack_result<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], UnpackResult, E>
where
    E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
{
    context(
        "unpack-result",
        alt((
            nom::combinator::map(tag(b"ok"), |_| UnpackResult::Ok),
            nom::combinator::map(parse_error_msg, UnpackResult::ErrorMsg),
        )),
    )(input)
}

// TODO: send commit without tree to trigger error for test case
fn parse_error_msg<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], ErrorMsg, E>
where
    E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
{
    context("error-msg", |input| {
        // FIXME: this should be 1*(OCTET)
        let (next_input, error_msg) =
            nom::combinator::verify(nom::combinator::rest, |bytes: &[u8]| {
                bytes.len() > 0 && bytes != b"ok"
            })(input)?;

        Ok((next_input, ErrorMsg(BString::from(error_msg))))
    })(input)
}

fn parse_command_status_v2<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], CommandStatusV2, E>
where
    E: nom::error::ParseError<&'a [u8]> + nom::error::ContextError<&'a [u8]>,
{
    context("command-status-v2", |input| todo!())(input)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    FailedToReadCommandStatus,
    FailedToReadUnpackStatus,
    Io(String),
    Nom(String),
    PacketLineDecode(String),
    UnexpectedFlush,
    UnexpectedDelimiter,
    UnexpectedResponseEnd,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::FailedToReadCommandStatus => "failed to read command status".to_string(),
            Self::FailedToReadUnpackStatus => "failed to read unpack status".to_string(),
            Self::Io(err) => format!("IO error: {}", err),
            Self::Nom(err) => format!("nom error: {}", err),
            Self::PacketLineDecode(err) => err.to_string(),
            Self::UnexpectedFlush => "unexpected flush packet".to_string(),
            Self::UnexpectedDelimiter => "unexpected delimiter".to_string(),
            Self::UnexpectedResponseEnd => "unexpected response end".to_string(),
        };
        write!(f, "{}", msg)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use git::bstr::ByteSlice;

    #[test]
    fn test_parse() {
        let input = vec!["000dunpack ok", "0016ok refs/heads/main", "0000"]
            .join("")
            .into_bytes();
        assert!(false)
    }

    #[test]
    fn test_parse_newlines() {
        let input = vec!["000eunpack ok\n", "0017ok refs/heads/main\n", "0000"]
            .join("")
            .into_bytes();
        assert!(false)
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
}
