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

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expected { expected, got } => write!(f, "Expected {expected}, found {got}"),
            Self::ExpectedAny { expected, got } => {
                f.write_str("Expected one of ")?;
                for (i, kind) in expected.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{kind}")?;
                }
                write!(f, ", found {got}")
            }
            Self::UnterminatedString => f.write_str("Unterminated string"),
            Self::Unexpected { token } => write!(f, "Unexpected {token}"),
            Self::UnexpectedTree => f.write_str("Unexpected tree"),
            Self::Custom(message) => f.write_str(message),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::UnexpectedTree => write!(f, "Unexpected {}", self.tree),
            kind => kind.fmt(f),
        }
    }
}
