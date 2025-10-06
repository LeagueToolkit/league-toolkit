use nom::{
    bytes::complete::escaped,
    character::complete::{char, none_of, one_of},
    sequence::delimited,
    IResult, Parser,
};

mod numeric;
pub use numeric::*;

use crate::ws;

pub fn string(input: &str) -> IResult<&str, &str> {
    ws(delimited(
        char('"'),
        escaped(none_of("\"\\"), '\\', one_of(r#"n"\"#)),
        char('"'),
    ))
    .parse(input)
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
