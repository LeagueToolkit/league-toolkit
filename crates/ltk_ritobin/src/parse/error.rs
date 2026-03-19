use crate::parse::{cst, tokenizer::TokenKind, Span};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    Expected {
        expected: TokenKind,
        got: TokenKind,
    },
    ExpectedAny {
        expected: &'static [TokenKind],
        got: TokenKind,
    },
    UnterminatedString,
    Unexpected {
        token: TokenKind,
    },
    /// When the entire tree we're in is unexpected
    UnexpectedTree,
    Custom(&'static str),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Copy)]
pub struct Error {
    pub span: Span,
    pub tree: cst::Kind,
    pub kind: ErrorKind,
}
