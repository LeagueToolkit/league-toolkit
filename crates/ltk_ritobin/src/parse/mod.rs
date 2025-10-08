use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{char, multispace0, multispace1},
    combinator::{recognize, value},
    error::ParseError,
    multi::many0,
    sequence::{delimited, preceded, terminated},
    IResult, Parser,
};
use nom_locate::LocatedSpan;
use statement::{statement, Statement};

pub mod literals;
pub mod statement;

/// A combinator that takes a parser `inner` and produces a parser that also consumes both leading and
/// trailing whitespace, returning the output of `inner`.
pub fn ws<'a, O, E: ParseError<Span<'a>>, F>(
    inner: F,
) -> impl Parser<Span<'a>, Output = O, Error = E>
where
    F: Parser<Span<'a>, Output = O, Error = E>,
{
    delimited(multispace0, inner, multispace0)
}

pub fn blank_space<'a, E: ParseError<Span<'a>>>(
) -> impl Parser<Span<'a>, Output = Span<'a>, Error = E> {
    recognize(many0(alt((multispace1, tag("\\\n")))))
}

pub fn comment(input: Span) -> IResult<Span, Span> {
    recognize((char('#'), is_not("\n\r"))).parse(input)
}
pub fn comments(input: Span) -> IResult<Span, Span> {
    recognize(many0(terminated(comment, blank_space()))).parse(input)
}
pub fn blank(input: Span) -> IResult<Span, ()> {
    value((), preceded(blank_space(), comments)).parse(input)
}

pub type Span<'a> = LocatedSpan<&'a str>;

pub fn parse(text: &str) -> IResult<Span, Vec<Statement>> {
    let text = Span::new(text);
    let mut statements = many0(preceded(blank, statement));

    let (input, stmts) = statements.parse(text)?;
    Ok((input, stmts))
}
