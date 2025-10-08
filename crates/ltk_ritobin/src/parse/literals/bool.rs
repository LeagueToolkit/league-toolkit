use nom::{branch::alt, bytes::complete::tag, IResult, Parser};

use crate::parse::{ws, Span};

pub fn boolean(input: Span) -> IResult<Span, (bool, Span)> {
    ws(alt((
        tag("true").map(|s| (true, s)),
        tag("false").map(|s| (false, s)),
    )))
    .parse(input)
}
