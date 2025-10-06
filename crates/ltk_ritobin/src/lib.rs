use std::io::{Read, Seek};

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

pub fn parse(text: &str) -> IResult<&str, &str> {
    let comment = recognize((char('#'), is_not("\n\r")));
    let comments = recognize(many0(terminated(comment, blank_space())));

    let blank = value((), preceded(blank_space(), comments));
    let bin_value = alt((
        literals::decimal,
        literals::float,
        literals::string,
        literals::binary,
        literals::hexadecimal,
        literals::octal,
    ));
    // let bin_value = literals::decimal;
    let statement = (
        ws(is_not("=:")), // name
        opt(preceded(char(':'), ws(bin_type))),
        preceded(char('='), ws(bin_value)),
    );

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
        let cases: [(&str, (&str, Option<Vec<&str>>)); 3] = [
            ("string", ("string", None)),
            ("u32", ("u32", None)),
            (" list[string]  ", ("list", Some(vec!["string"]))),
        ];
        for (test, out) in cases {
            let (_, got) = bin_type(test).unwrap();
            assert_eq!(out, got);
        }
    }
}
