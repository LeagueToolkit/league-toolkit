use super::literals::{literal, Literal};

use nom::{
    branch::alt,
    bytes::complete::take_till,
    character::complete::{alphanumeric1, char},
    combinator::{consumed, opt},
    multi::separated_list1,
    sequence::{delimited, preceded},
    IResult, Parser,
};

use super::{ws, Span};
pub fn type_definition(input: Span) -> IResult<Span, TypeDefinition> {
    ws(consumed((
        alphanumeric1,
        opt(delimited(
            char('['),
            consumed(separated_list1(ws(char(',')), alphanumeric1)),
            char(']'),
        )),
    )))
    .map(|(span, (base, subtypes))| TypeDefinition {
        span,
        base,
        subtypes,
    })
    .parse(input)
}

#[derive(Debug, Clone)]
pub struct TypeDefinition<'a> {
    /// The full type definition's span
    pub span: Span<'a>,
    pub base: Span<'a>,
    pub subtypes: Option<(Span<'a>, Vec<Span<'a>>)>,
}

#[derive(Debug, Clone)]
pub struct Statement<'a> {
    pub name: Literal<'a>,
    pub kind: Option<TypeDefinition<'a>>,
    pub value: Literal<'a>,
}

pub fn statement(input: Span) -> IResult<Span, Statement<'_>> {
    let (input, (name, kind, value)) = (
        alt((
            ws(literal),
            ws(take_till(|c: char| {
                c.is_whitespace() || c == ':' || c == '='
            }))
            .map(Literal::Keyword),
        )), // name
        opt(preceded(ws(char(':')), ws(type_definition))),
        preceded(ws(char('=')), ws(literal)),
    )
        .parse(input)?;

    Ok((input, Statement { name, kind, value }))
}

// #[cfg(test)]
// mod tests {
//     use super::type_definition;
//     use crate::parse::Span;
//
//     #[test]
//     fn type_definitions() {
//         #[allow(clippy::type_complexity)]
//         let cases: [(&str, (&str, Option<Vec<&str>>)); 4] = [
//             ("string", ("string", None)),
//             ("u32", ("u32", None)),
//             (" list[string]  ", ("list", Some(vec!["string"]))),
//             ("map[hash,embed]", ("map", Some(vec!["hash", "embed"]))),
//         ];
//         for (test, (out_base, out_children)) in cases {
//             let test = Span::new(test);
//             let (_, (base, children)) = type_definition(test).unwrap();
//             assert_eq!(out_base, base.into_fragment());
//             assert_eq!(
//                 out_children,
//                 children.map(|c| c.into_iter().map(|c| c.into_fragment()).collect())
//             );
//         }
//     }
// }
