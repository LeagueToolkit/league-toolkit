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

use crate::ws;

pub fn string(input: &str) -> IResult<&str, &str> {
    ws(delimited(
        char('"'),
        opt(escaped(none_of("\"\\"), '\\', one_of(r#"n"\"#))).map(|v| v.unwrap_or_default()),
        char('"'),
    ))
    .parse(input)
}

pub fn boolean(input: &str) -> IResult<&str, bool> {
    ws(alt((value(true, tag("true")), value(false, tag("false"))))).parse(input)
}

#[cfg(test)]
pub mod tests {
    use super::string;

    #[test]
    fn string_lit_1() {
        let input = r#"  "my 40 cool strings are very cooo!!!_--=z-9-021391 23'''; \" \" "  "#;
        let (_, str) = string(input).unwrap();
        assert_eq!(
            str,
            "my 40 cool strings are very cooo!!!_--=z-9-021391 23'''; \\\" \\\" "
        );
    }
}
