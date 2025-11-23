use std::fmt::Display;

use enum_kinds::EnumKind;
use nom::{branch::alt, IResult, Parser};

use crate::parse::Span;

mod numeric;
pub use numeric::*;

mod block;
pub use block::*;

mod string;
pub use string::*;

mod bool;
pub use bool::*;

#[derive(Debug, Clone, EnumKind)]
#[enum_kind(LiteralKind)]
pub enum Literal<'a> {
    Block(Block<'a>),
    Keyword(Span<'a>),
    String(Option<Span<'a>>),

    Decimal(Span<'a>),
    Hexadecimal(Span<'a>),
    Octal(Span<'a>),
    Binary(Span<'a>),

    Bool(bool, Span<'a>),
}

impl Display for LiteralKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LiteralKind::Block => "block",
            LiteralKind::Keyword => "keyword",
            LiteralKind::String => "string",
            LiteralKind::Decimal => "decimal number",
            LiteralKind::Hexadecimal => "hex number",
            LiteralKind::Octal => "octal number",
            LiteralKind::Binary => "binary number",
            LiteralKind::Bool => "bool",
        })
    }
}

impl<'a> Literal<'a> {
    pub fn kind(&self) -> LiteralKind {
        self.into()
    }
    pub fn span(&self) -> &Span<'a> {
        match self {
            Literal::Block(block) => &block.span,
            Literal::String(span) => span.as_ref().expect("TODO: empty string spans"),
            Literal::Keyword(span)
            | Literal::Decimal(span)
            | Literal::Hexadecimal(span)
            | Literal::Octal(span)
            | Literal::Binary(span)
            | Literal::Bool(_, span) => span,
        }
    }
}

pub fn literal(input: Span) -> IResult<Span, Literal<'_>> {
    alt((
        boolean.map(|(b, s)| Literal::Bool(b, s)),
        string.map(Literal::String),
        hexadecimal.map(Literal::Hexadecimal),
        binary.map(Literal::Binary),
        octal.map(Literal::Octal),
        float.map(Literal::Decimal),
        integer.map(Literal::Decimal),
        block.map(Literal::Block),
    ))
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
