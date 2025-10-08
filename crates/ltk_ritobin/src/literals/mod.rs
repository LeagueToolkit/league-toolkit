use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::complete::{char, none_of, one_of},
    combinator::{opt, value},
    sequence::delimited,
    IResult, Parser,
};

mod numeric;
pub use numeric::*;

mod block;
pub use block::*;

use crate::{ws, Span};

pub fn string(input: Span) -> IResult<Span, Option<Span>> {
    ws(delimited(
        char('"'),
        opt(escaped(none_of("\"\\"), '\\', one_of(r#"n"\"#))),
        char('"'),
    ))
    .parse(input)
}

pub fn boolean(input: Span) -> IResult<Span, (bool, Span)> {
    ws(alt((
        tag("true").map(|s| (true, s)),
        tag("false").map(|s| (false, s)),
    )))
    .parse(input)
}

#[cfg(test)]
pub mod tests {
    use crate::Span;

    use super::string;

    #[test]
    fn string_lit_1() {
        let input =
            Span::new(r#"  "my 40 cool strings are very cooo!!!_--=z-9-021391 23'''; \" \" "  "#);
        let (_, str) = string(input).unwrap();
        assert_eq!(
            str.map(|s| s.into_fragment()),
            Some("my 40 cool strings are very cooo!!!_--=z-9-021391 23'''; \\\" \\\" ")
        );
    }
}
