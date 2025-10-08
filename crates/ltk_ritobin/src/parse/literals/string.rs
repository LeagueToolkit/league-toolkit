use nom::{
    bytes::complete::escaped,
    character::complete::{char, none_of, one_of},
    combinator::opt,
    sequence::delimited,
    IResult, Parser,
};

use crate::parse::{ws, Span};

pub fn string(input: Span) -> IResult<Span, Option<Span>> {
    ws(delimited(
        char('"'),
        opt(escaped(none_of("\"\\"), '\\', one_of(r#"n"\"#))),
        char('"'),
    ))
    .parse(input)
}
