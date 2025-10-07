use std::io::{Read, Seek};

use literals::Block;
use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag, take_till, take_until},
    character::complete::{
        alphanumeric0, alphanumeric1, anychar, char, multispace0, multispace1, none_of, one_of,
    },
    combinator::{opt, recognize, value},
    error::ParseError,
    multi::{many0, separated_list1},
    sequence::{delimited, preceded, separated_pair, terminated, tuple},
    AsChar, IResult, Parser,
};

mod literals;

/// A combinator that takes a parser `inner` and produces a parser that also consumes both leading and
/// trailing whitespace, returning the output of `inner`.
pub fn ws<'a, O, E: ParseError<&'a str>, F>(inner: F) -> impl Parser<&'a str, Output = O, Error = E>
where
    F: Parser<&'a str, Output = O, Error = E>,
{
    delimited(multispace0, inner, multispace0)
}

pub fn blank_space<'a, E: ParseError<&'a str>>() -> impl Parser<&'a str, Output = &'a str, Error = E>
{
    recognize(many0(alt((multispace1, tag("\\\n")))))
}

pub fn bin_type(input: &str) -> IResult<&str, (&str, Option<Vec<&str>>)> {
    ws((
        alphanumeric1,
        opt(delimited(
            char('['),
            separated_list1(ws(char(',')), alphanumeric1),
            char(']'),
        )),
    ))
    .parse(input)
}

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Block(Block<'a>),
    Keyword(&'a str),
    String(&'a str),
    Bool(bool),
    Other(&'a str),
}

pub fn bin_value(input: &str) -> IResult<&str, Value<'_>> {
    alt((
        literals::boolean.map(Value::Bool),
        literals::string.map(Value::String),
        literals::hexadecimal.map(Value::Other),
        literals::binary.map(Value::Other),
        literals::octal.map(Value::Other),
        literals::float.map(Value::Other),
        literals::integer.map(Value::Other),
        literals::block.map(Value::Block),
    ))
    .parse(input)
}

#[derive(Debug, Clone)]
pub struct Type<'a> {
    pub value: &'a str,
    pub subtypes: Option<Vec<&'a str>>,
}

#[derive(Debug, Clone)]
pub struct Statement<'a> {
    pub name: Value<'a>,
    pub kind: Option<Type<'a>>,
    pub value: Value<'a>,
}

pub fn statement(input: &str) -> IResult<&str, Statement<'_>> {
    let (input, (name, kind, value)) = (
        alt((
            ws(literals::string).map(Value::String),
            ws(literals::hexadecimal).map(Value::Other),
            ws(take_till(|c: char| {
                c.is_whitespace() || c == ':' || c == '='
            }))
            .map(Value::Keyword),
        )), // name
        opt(preceded(ws(char(':')), ws(bin_type))),
        preceded(ws(char('=')), ws(bin_value)),
    )
        .parse(input)?;

    Ok((
        input,
        Statement {
            name,
            kind: kind.map(|(value, subtypes)| Type { value, subtypes }),
            value,
        },
    ))
}

pub fn comment(input: &str) -> IResult<&str, &str> {
    recognize((char('#'), is_not("\n\r"))).parse(input)
}
pub fn comments(input: &str) -> IResult<&str, &str> {
    recognize(many0(terminated(comment, blank_space()))).parse(input)
}
pub fn blank(input: &str) -> IResult<&str, ()> {
    value((), preceded(blank_space(), comments)).parse(input)
}

pub fn parse(text: &str) -> IResult<&str, &str> {
    let mut statements = many0((blank, statement));

    let (input, stmt) = statements.parse(text)?;
    println!("stmt: {stmt:#?}");
    println!("left: {:?}", input.split_once('\n').map(|a| a.0));
    Ok((input, input))
}

#[cfg(test)]
mod tests {
    use crate::bin_type;

    #[test]
    fn bin_types() {
        #[allow(clippy::type_complexity)]
        let cases: [(&str, (&str, Option<Vec<&str>>)); 4] = [
            ("string", ("string", None)),
            ("u32", ("u32", None)),
            (" list[string]  ", ("list", Some(vec!["string"]))),
            ("map[hash,embed]", ("map", Some(vec!["hash", "embed"]))),
        ];
        for (test, out) in cases {
            let (_, got) = bin_type(test).unwrap();
            assert_eq!(out, got);
        }
    }
}
