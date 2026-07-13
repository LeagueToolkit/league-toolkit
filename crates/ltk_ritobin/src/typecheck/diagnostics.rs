use std::fmt::Display;

use ltk_meta::PropertyKind;

use crate::{
    cst,
    parse::{Span, TokenKind},
    RitoType,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RootKind {
    Type,
    Version,
    Linked,
    Entries,
}

#[derive(Debug, Clone, Copy)]
pub enum RitoTypeOrVirtual {
    RitoType(RitoType),
    Numeric,
    StructOrEmbedded,
    Token(TokenKind),
    Tree(cst::Kind),
}

impl RitoTypeOrVirtual {
    pub fn numeric() -> Self {
        Self::Numeric
    }
}

impl From<RitoType> for RitoTypeOrVirtual {
    fn from(value: RitoType) -> Self {
        RitoTypeOrVirtual::RitoType(value)
    }
}

impl Display for RitoTypeOrVirtual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RitoType(rito_type) => Display::fmt(rito_type, f),
            Self::Numeric => f.write_str("numeric type"),
            Self::StructOrEmbedded => f.write_str("struct/embedded"),
            Self::Token(kind) => Display::fmt(kind, f),
            Self::Tree(kind) => Display::fmt(kind, f),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ColorOrVec {
    Color,
    Vec2,
    Vec3,
    Vec4,
    Mat44,
}

#[derive(Debug, Clone, Copy)]
pub enum Diagnostic {
    CustomSpan(&'static str, Span),

    UnexpectedTree {
        tree: cst::Kind,
        expected: Option<cst::Kind>,
        span: Span,
    },
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),

    MissingToken(TokenKind),
    UnknownType(Span),
    MissingType(Span),

    MissingRootEntry {
        root_kind: RootKind,
    },

    InvalidRootEntryType {
        root_kind: RootKind,
        key_span: Span,
        type_span: Span,
        got: RitoType,
        expected: RitoType,
    },

    TypeMismatch {
        span: Span,
        expected: RitoType,
        expected_span: Option<Span>,
        got: RitoTypeOrVirtual,
    },

    UnexpectedContainerItem {
        span: Span,
        expected: RitoType,
        expected_span: Option<Span>,
    },

    ResolveLiteral,
    ParseNumericError {
        expected: PropertyKind,
        error: Option<std::num::IntErrorKind>,
        span: Span,
    },
    AmbiguousNumeric(Span),

    NotEnoughItems {
        span: Span,
        got: u8,
        expected: ColorOrVec,
    },
    TooManyItems {
        span: Span,
        extra: u8,
        expected: ColorOrVec,
    },

    /// Root entry is not a valid entry (key: type = value)
    RootNonEntry,
    /// Root entry is not recognised
    UnknownRoot {
        /// span of the unrecognised entry's name
        span: Span,
    },
    ShadowedEntry {
        shadowee: Span,
        shadower: Span,
    },

    InvalidHash(Span),

    SubtypeCountMismatch {
        span: Span,
        got: u8,
        expected: u8,
    },
    /// Subtypes found on a type that has no subtypes
    UnexpectedSubtypes {
        span: Span,
        base_type: Span,
    },
}

impl Diagnostic {
    pub fn span(&self) -> Option<&Span> {
        use Diagnostic::*;
        match self {
            MissingTree(_)
            | EmptyTree(_)
            | MissingToken(_)
            | RootNonEntry
            | ResolveLiteral
            | MissingRootEntry { .. } => None,
            UnknownType(span)
            | UnknownRoot { span }
            | UnexpectedTree { span, .. }
            | CustomSpan(_, span)
            | SubtypeCountMismatch { span, .. }
            | UnexpectedSubtypes { span, .. }
            | UnexpectedContainerItem { span, .. }
            | MissingType(span)
            | TypeMismatch { span, .. }
            | ShadowedEntry { shadower: span, .. }
            | InvalidHash(span)
            | AmbiguousNumeric(span)
            | ParseNumericError { span, .. }
            | NotEnoughItems { span, .. }
            | TooManyItems { span, .. }
            | InvalidRootEntryType { key_span: span, .. } => Some(span),
        }
    }

    pub fn default_span(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap_or(span),
            diagnostic: self,
        }
    }

    pub fn unwrap(self) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap(),
            diagnostic: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiagnosticWithSpan {
    pub diagnostic: Diagnostic,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct MaybeSpanDiag {
    pub diagnostic: Diagnostic,
    pub span: Option<Span>,
}

impl MaybeSpanDiag {
    pub fn fallback(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span.unwrap_or(span),
            diagnostic: self.diagnostic,
        }
    }
}

impl From<Diagnostic> for MaybeSpanDiag {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            span: diagnostic.span().copied(),
            diagnostic,
        }
    }
}
