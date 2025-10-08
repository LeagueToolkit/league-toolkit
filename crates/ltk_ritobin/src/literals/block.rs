use crate::{bin_value, blank, statement, ws, Span, Statement, Value};
use nom::{
    branch::alt,
    character::complete::{alpha1, char},
    combinator::{consumed, opt},
    multi::many1,
    sequence::{delimited, preceded},
    IResult, Parser,
};

#[derive(Debug, Clone)]
pub struct Block<'a> {
    pub span: Span<'a>,
    pub class: Option<Span<'a>>,
    pub inner: BlockContent<'a>,
}

#[derive(Debug, Clone)]
pub enum BlockContent<'a> {
    Empty,
    Statements(Vec<Statement<'a>>),
    Values(Vec<Value<'a>>),
}
pub fn block(input: Span) -> IResult<Span, Block> {
    consumed((
        ws(opt(alpha1)),
        delimited(
            delimited(blank, char('{'), blank),
            opt(alt((
                many1(preceded(blank, statement)).map(BlockContent::Statements),
                many1(preceded(blank, bin_value)).map(BlockContent::Values),
            ))),
            preceded(blank, char('}')),
        ),
    ))
    .map(|(span, (class, statements))| Block {
        span,
        class,
        inner: statements.unwrap_or(BlockContent::Empty),
    })
    .parse(input)
}
