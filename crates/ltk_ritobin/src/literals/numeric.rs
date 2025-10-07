use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, one_of},
    combinator::{opt, recognize},
    multi::{many0, many1},
    sequence::{preceded, terminated},
    IResult, Parser,
};

pub fn hexadecimal(input: &str) -> IResult<&str, &str> {
    preceded(
        alt((tag("0x"), tag("0X"))),
        recognize(many1(terminated(
            one_of("0123456789abcdefABCDEF"),
            many0(char('_')),
        ))),
    )
    .parse(input)
}

pub fn octal(input: &str) -> IResult<&str, &str> {
    preceded(
        alt((tag("0o"), tag("0O"))),
        recognize(many1(terminated(one_of("01234567"), many0(char('_'))))),
    )
    .parse(input)
}

pub fn binary(input: &str) -> IResult<&str, &str> {
    preceded(
        alt((tag("0b"), tag("0B"))),
        recognize(many1(terminated(one_of("01"), many0(char('_'))))),
    )
    .parse(input)
}

pub fn float(input: &str) -> IResult<&str, &str> {
    alt((
        // Case one: .42
        recognize((
            opt(char('-')),
            char('.'),
            decimal,
            opt((one_of("eE"), opt(one_of("+-")), decimal)),
        )), // Case two: 42e42 and 42.42e42
        recognize((
            opt(char('-')),
            decimal,
            opt(preceded(char('.'), decimal)),
            one_of("eE"),
            opt(one_of("+-")),
            decimal,
        )), // Case three: 42. and 42.42
        recognize((opt(char('-')), decimal, char('.'), opt(decimal))),
    ))
    .parse(input)
}

pub fn integer(input: &str) -> IResult<&str, &str> {
    recognize((opt(char('-')), decimal)).parse(input)
}

pub fn decimal(input: &str) -> IResult<&str, &str> {
    recognize((many1(terminated(one_of("0123456789"), many0(char('_')))),)).parse(input)
}
