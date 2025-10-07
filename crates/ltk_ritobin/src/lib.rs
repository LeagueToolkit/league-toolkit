use literals::Block;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till},
    character::complete::{alphanumeric1, char, multispace0, multispace1},
    combinator::{opt, recognize, value},
    error::ParseError,
    multi::{many0, separated_list1},
    sequence::{delimited, preceded, terminated},
    IResult, Parser,
};
use nom_locate::LocatedSpan;

mod literals;

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

pub fn bin_type(input: Span) -> IResult<Span, (Span, Option<Vec<Span>>)> {
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
    Keyword(Span<'a>),
    String(Option<Span<'a>>),

    Decimal(Span<'a>),
    Hexadecimal(Span<'a>),
    Octal(Span<'a>),
    Binary(Span<'a>),

    Bool(bool),
}

pub fn bin_value(input: Span) -> IResult<Span, Value<'_>> {
    alt((
        literals::boolean.map(Value::Bool),
        literals::string.map(Value::String),
        literals::hexadecimal.map(Value::Hexadecimal),
        literals::binary.map(Value::Binary),
        literals::octal.map(Value::Octal),
        literals::float.map(Value::Decimal),
        literals::integer.map(Value::Decimal),
        literals::block.map(Value::Block),
    ))
    .parse(input)
}

#[derive(Debug, Clone)]
pub struct Type<'a> {
    pub value: Span<'a>,
    pub subtypes: Option<Vec<Span<'a>>>,
}

#[derive(Debug, Clone)]
pub struct Statement<'a> {
    pub name: Value<'a>,
    pub kind: Option<Type<'a>>,
    pub value: Value<'a>,
}

pub fn statement(input: Span) -> IResult<Span, Statement<'_>> {
    let (input, (name, kind, value)) = (
        alt((
            ws(literals::string).map(Value::String),
            ws(literals::hexadecimal).map(Value::Hexadecimal),
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

pub fn comment(input: Span) -> IResult<Span, Span> {
    recognize((char('#'), is_not("\n\r"))).parse(input)
}
pub fn comments(input: Span) -> IResult<Span, Span> {
    recognize(many0(terminated(comment, blank_space()))).parse(input)
}
pub fn blank(input: Span) -> IResult<Span, ()> {
    value((), preceded(blank_space(), comments)).parse(input)
}

type Span<'a> = LocatedSpan<&'a str>;
pub fn parse(text: &str) -> IResult<Span, Span> {
    let text = Span::new(text);
    let mut statements = many0(preceded(blank, statement));

    let (input, stmts) = statements.parse(text)?;
    println!("stmt: {stmts:#?}");
    println!("left: {:?}", input.split_once('\n').map(|a| a.0));
    Ok((input, input))
}

#[cfg(test)]
mod tests {
    use crate::{bin_type, Span};

    #[test]
    fn bin_types() {
        #[allow(clippy::type_complexity)]
        let cases: [(&str, (&str, Option<Vec<&str>>)); 4] = [
            ("string", ("string", None)),
            ("u32", ("u32", None)),
            (" list[string]  ", ("list", Some(vec!["string"]))),
            ("map[hash,embed]", ("map", Some(vec!["hash", "embed"]))),
        ];
        for (test, (out_base, out_children)) in cases {
            let test = Span::new(test);
            let (_, (base, children)) = bin_type(test).unwrap();
            assert_eq!(out_base, base.into_fragment());
            assert_eq!(
                out_children,
                children.map(|c| c.into_iter().map(|c| c.into_fragment()).collect())
            );
        }
    }
}
