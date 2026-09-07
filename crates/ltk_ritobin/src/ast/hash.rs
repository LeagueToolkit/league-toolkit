use std::fmt::Display;

use crate::{parse::Span, Spanned};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HashedLiteral<H: ltk_hash::Hash> {
    pub value: H,
    /// What this hash was originally, before being coerced to a hash
    pub originally: Spanned<Originally>,
}

/// See [`HashedLiteral::originally`] for information.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Originally {
    /// A `HexLit` token (`0xdeadbeef`), its value was used directly & no coercion was needed
    #[default]
    HexLit,
    /// A `String` token (`"hello"`), its text value was hashed
    /// (i.e. with all escapes resolved & quotes excluded)
    String,
    /// A `Name` token (`helloWorld`), the text of that token was hashed
    Name,
}

impl<H: ltk_hash::Hash> HashedLiteral<H> {
    #[inline(always)]
    #[must_use]
    pub fn new(span: Span, originally: Originally, value: H) -> Self {
        Self {
            value,
            originally: Spanned::new(span, originally),
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.originally.span = span;
        self
    }

    #[inline(always)]
    #[must_use]
    pub fn with_value<NewHash: ltk_hash::Hash>(self, value: NewHash) -> HashedLiteral<NewHash> {
        HashedLiteral {
            value,
            originally: self.originally,
        }
    }

    /// The span of the originating token that coerced to this hash
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        self.originally.span
    }

    /// What this hash was originally, before being coerced to a hash
    #[inline(always)]
    #[must_use]
    pub fn original_kind(&self) -> Originally {
        self.originally.value
    }

    #[inline(always)]
    #[must_use]
    /// Whether this hash was originally a hash literal (`HexLit` token)
    pub fn was_hash(&self) -> bool {
        matches!(self.original_kind(), Originally::HexLit)
    }
    #[inline(always)]
    #[must_use]
    /// Whether this hash was originally a string literal, like for a property key (`String` token)
    pub fn was_str(&self) -> bool {
        matches!(self.original_kind(), Originally::String)
    }
    #[inline(always)]
    #[must_use]
    /// Whether this hash was originally a raw name, like for a class hash (`Name` token)
    pub fn was_name(&self) -> bool {
        matches!(self.original_kind(), Originally::Name)
    }
}

impl<H: ltk_hash::Hash + Display> Display for HashedLiteral<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", self.value)
    }
}
